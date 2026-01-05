use std::{future::Future, time::Duration};

use sqlx::Error;

const MAX_RETRIES: usize = 3;
const INITIAL_BACKOFF_MS: u64 = 50;
const MAX_BACKOFF_MS: u64 = 1_000;

pub(crate) async fn retry_on_sqlite_busy<T, F, Fut>(mut op: F) -> Result<T, Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let mut backoff = Duration::from_millis(INITIAL_BACKOFF_MS);
    for attempt in 0..=MAX_RETRIES {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) if is_sqlite_busy(&err) && attempt < MAX_RETRIES => {
                tokio::time::sleep(backoff).await;
                let next_ms = (backoff.as_millis() as u64)
                    .saturating_mul(2)
                    .min(MAX_BACKOFF_MS);
                backoff = Duration::from_millis(next_ms);
            }
            Err(err) => return Err(err),
        }
    }

    unreachable!("retry loop returns on success or error")
}

fn is_sqlite_busy(err: &Error) -> bool {
    let Some(db_err) = err.as_database_error() else {
        return false;
    };

    if let Some(code) = db_err.code() {
        if code == "5" || code == "6" {
            return true;
        }
    }

    let message = db_err.message();
    message.contains("database is locked") || message.contains("database is busy")
}

#[cfg(test)]
mod tests {
    use super::retry_on_sqlite_busy;
    use crate::models::{
        execution_process::{ExecutionProcess, ExecutionProcessStatus},
        task::{Task, TaskStatus},
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::{
        path::PathBuf,
        str::FromStr,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };
    use tokio::sync::oneshot;
    use uuid::Uuid;

    async fn setup_pool(run_migrations: bool) -> Result<(sqlx::SqlitePool, PathBuf), sqlx::Error>
    {
        let db_path =
            std::env::temp_dir().join(format!("vk-retry-test-{}.db", Uuid::new_v4()));
        let db_url = format!("sqlite://{}", db_path.to_string_lossy());
        let options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true)
            .busy_timeout(Duration::from_millis(0));
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await?;

        if run_migrations {
            sqlx::migrate!("./migrations").run(&pool).await?;
        }

        Ok((pool, db_path))
    }

    fn cleanup_db(db_path: PathBuf) {
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    async fn lock_table_for_duration(
        pool: sqlx::SqlitePool,
        sql: &'static str,
        bind_id: Uuid,
        hold_ms: u64,
        tx: oneshot::Sender<()>,
    ) -> Result<(), sqlx::Error> {
        let mut conn = pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE;")
            .execute(&mut *conn)
            .await?;
        sqlx::query(sql).bind(bind_id).execute(&mut *conn).await?;
        let _ = tx.send(());
        tokio::time::sleep(Duration::from_millis(hold_ms)).await;
        sqlx::query("COMMIT;").execute(&mut *conn).await?;
        Ok(())
    }

    #[tokio::test]
    async fn retries_when_database_is_locked() -> Result<(), sqlx::Error> {
        let (pool, db_path) = setup_pool(false).await?;

        sqlx::query("CREATE TABLE test_lock (id INTEGER PRIMARY KEY, v INTEGER NOT NULL);")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO test_lock (id, v) VALUES (1, 0);")
            .execute(&pool)
            .await?;

        let pool_for_lock = pool.clone();
        let (tx, rx) = oneshot::channel();
        let lock_task = tokio::spawn(async move {
            let mut conn = pool_for_lock.acquire().await.expect("acquire lock conn");
            sqlx::query("BEGIN IMMEDIATE;")
                .execute(&mut *conn)
                .await
                .expect("begin immediate");
            sqlx::query("UPDATE test_lock SET v = v + 1 WHERE id = 1;")
                .execute(&mut *conn)
                .await
                .expect("update under lock");
            let _ = tx.send(());
            tokio::time::sleep(Duration::from_millis(200)).await;
            sqlx::query("COMMIT;")
                .execute(&mut *conn)
                .await
                .expect("commit lock");
        });

        rx.await.expect("lock acquired");

        let attempts = AtomicUsize::new(0);
        retry_on_sqlite_busy(|| {
            attempts.fetch_add(1, Ordering::SeqCst);
            async {
                sqlx::query("UPDATE test_lock SET v = v + 1 WHERE id = 1;")
                    .execute(&pool)
                    .await?;
                Ok(())
            }
        })
        .await?;

        lock_task.await.expect("lock task complete");

        let final_value: i64 =
            sqlx::query_scalar("SELECT v FROM test_lock WHERE id = 1;")
                .fetch_one(&pool)
                .await?;
        assert_eq!(final_value, 2);
        assert!(attempts.load(Ordering::SeqCst) > 1);

        drop(pool);
        cleanup_db(db_path);

        Ok(())
    }

    #[tokio::test]
    async fn task_update_status_retries_on_lock() -> Result<(), sqlx::Error> {
        let (pool, db_path) = setup_pool(true).await?;

        let project_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2);")
            .bind(project_id)
            .bind("test-project")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO tasks (id, project_id, title) VALUES ($1, $2, $3);")
            .bind(task_id)
            .bind(project_id)
            .bind("test-task")
            .execute(&pool)
            .await?;

        let pool_for_lock = pool.clone();
        let (tx, rx) = oneshot::channel();
        let lock_task = tokio::spawn(lock_table_for_duration(
            pool_for_lock,
            "UPDATE tasks SET title = title WHERE id = $1;",
            task_id,
            200,
            tx,
        ));

        rx.await.expect("lock acquired");

        Task::update_status(&pool, task_id, TaskStatus::InReview).await?;

        lock_task.await.expect("lock task")?;

        let status: String =
            sqlx::query_scalar("SELECT status FROM tasks WHERE id = $1;")
                .bind(task_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(status, "inreview");

        drop(pool);
        cleanup_db(db_path);

        Ok(())
    }

    #[tokio::test]
    async fn execution_process_update_completion_retries_on_lock() -> Result<(), sqlx::Error> {
        let (pool, db_path) = setup_pool(true).await?;

        let project_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let process_id = Uuid::new_v4();
        sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2);")
            .bind(project_id)
            .bind("test-project")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO tasks (id, project_id, title) VALUES ($1, $2, $3);")
            .bind(task_id)
            .bind(project_id)
            .bind("test-task")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO workspaces (id, task_id, branch) VALUES ($1, $2, $3);")
            .bind(workspace_id)
            .bind(task_id)
            .bind("main")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ($1, $2);")
            .bind(session_id)
            .bind(workspace_id)
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO execution_processes (id, session_id) VALUES ($1, $2);")
            .bind(process_id)
            .bind(session_id)
            .execute(&pool)
            .await?;

        let pool_for_lock = pool.clone();
        let (tx, rx) = oneshot::channel();
        let lock_task = tokio::spawn(lock_table_for_duration(
            pool_for_lock,
            "UPDATE execution_processes SET status = status WHERE id = $1;",
            process_id,
            200,
            tx,
        ));

        rx.await.expect("lock acquired");

        ExecutionProcess::update_completion(
            &pool,
            process_id,
            ExecutionProcessStatus::Completed,
            Some(0),
        )
        .await?;

        lock_task.await.expect("lock task")?;

        let status: String =
            sqlx::query_scalar("SELECT status FROM execution_processes WHERE id = $1;")
                .bind(process_id)
                .fetch_one(&pool)
                .await?;
        assert_eq!(status, "completed");

        drop(pool);
        cleanup_db(db_path);

        Ok(())
    }
}
