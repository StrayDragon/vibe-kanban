use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use utils::log_entries::LogEntryChannel;
use uuid::Uuid;

use crate::retry::retry_on_sqlite_busy;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ExecutionProcessLogEntry {
    pub execution_id: Uuid,
    pub channel: String,
    pub entry_index: i64,
    pub entry_json: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LogEntryRow {
    pub entry_index: i64,
    pub entry_json: String,
}

#[derive(Debug, Clone)]
pub struct LogEntryStats {
    pub count: i64,
    pub min_index: i64,
    pub max_index: i64,
}

#[derive(Debug, Clone, FromRow)]
struct LogEntryStatsRow {
    pub count: i64,
    pub min_index: Option<i64>,
    pub max_index: Option<i64>,
}

impl ExecutionProcessLogEntry {
    pub async fn stats(
        pool: &SqlitePool,
        execution_id: Uuid,
        channel: LogEntryChannel,
    ) -> Result<Option<LogEntryStats>, sqlx::Error> {
        let channel_value = channel.as_str();
        let row = sqlx::query_as!(
            LogEntryStatsRow,
            r#"SELECT COUNT(*) as "count!: i64",
                      MIN(entry_index) as "min_index: i64",
                      MAX(entry_index) as "max_index: i64"
               FROM execution_process_log_entries
               WHERE execution_id = $1 AND channel = $2"#,
            execution_id,
            channel_value,
        )
        .fetch_one(pool)
        .await?;

        if row.count == 0 {
            return Ok(None);
        }

        let min_index = row.min_index.unwrap_or(0);
        let max_index = row.max_index.unwrap_or(min_index);
        Ok(Some(LogEntryStats {
            count: row.count,
            min_index,
            max_index,
        }))
    }

    pub async fn has_any(
        pool: &SqlitePool,
        execution_id: Uuid,
        channel: LogEntryChannel,
    ) -> Result<bool, sqlx::Error> {
        let channel_value = channel.as_str();
        let exists = sqlx::query_scalar!(
            r#"SELECT 1 as "one!: i64"
               FROM execution_process_log_entries
               WHERE execution_id = $1 AND channel = $2
               LIMIT 1"#,
            execution_id,
            channel_value,
        )
        .fetch_optional(pool)
        .await?
        .is_some();

        Ok(exists)
    }

    pub async fn fetch_page(
        pool: &SqlitePool,
        execution_id: Uuid,
        channel: LogEntryChannel,
        limit: usize,
        cursor: Option<i64>,
    ) -> Result<Vec<LogEntryRow>, sqlx::Error> {
        let channel_value = channel.as_str();
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = if let Some(cursor) = cursor {
            sqlx::query_as!(
                LogEntryRow,
                r#"SELECT entry_index, entry_json
                   FROM execution_process_log_entries
                   WHERE execution_id = $1
                     AND channel = $2
                     AND entry_index < $3
                   ORDER BY entry_index DESC
                   LIMIT $4"#,
                execution_id,
                channel_value,
                cursor,
                limit,
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as!(
                LogEntryRow,
                r#"SELECT entry_index, entry_json
                   FROM execution_process_log_entries
                   WHERE execution_id = $1
                     AND channel = $2
                   ORDER BY entry_index DESC
                   LIMIT $3"#,
                execution_id,
                channel_value,
                limit,
            )
            .fetch_all(pool)
            .await?
        };

        Ok(rows)
    }

    pub async fn has_older(
        pool: &SqlitePool,
        execution_id: Uuid,
        channel: LogEntryChannel,
        before_index: i64,
    ) -> Result<bool, sqlx::Error> {
        let channel_value = channel.as_str();
        let exists = sqlx::query_scalar!(
            r#"SELECT 1 as "one!: i64"
               FROM execution_process_log_entries
               WHERE execution_id = $1
                 AND channel = $2
                 AND entry_index < $3
               LIMIT 1"#,
            execution_id,
            channel_value,
            before_index,
        )
        .fetch_optional(pool)
        .await?
        .is_some();

        Ok(exists)
    }

    pub async fn upsert_entry(
        pool: &SqlitePool,
        execution_id: Uuid,
        channel: LogEntryChannel,
        entry_index: i64,
        entry_json: &str,
    ) -> Result<(), sqlx::Error> {
        let channel_value = channel.as_str();
        retry_on_sqlite_busy(|| async {
            sqlx::query!(
                r#"INSERT INTO execution_process_log_entries (
                       execution_id, channel, entry_index, entry_json
                   ) VALUES ($1, $2, $3, $4)
                   ON CONFLICT(execution_id, channel, entry_index)
                   DO UPDATE SET
                     entry_json = excluded.entry_json,
                     updated_at = datetime('now', 'subsec')"#,
                execution_id,
                channel_value,
                entry_index,
                entry_json,
            )
            .execute(pool)
            .await?;
            Ok(())
        })
        .await?;

        Ok(())
    }

    pub async fn upsert_entries(
        pool: &SqlitePool,
        execution_id: Uuid,
        channel: LogEntryChannel,
        entries: &[LogEntryRow],
    ) -> Result<(), sqlx::Error> {
        if entries.is_empty() {
            return Ok(());
        }

        retry_on_sqlite_busy(|| async {
            let channel_value = channel.as_str();
            let mut query_builder = sqlx::QueryBuilder::new(
                "INSERT INTO execution_process_log_entries (execution_id, channel, entry_index, entry_json) ",
            );

            query_builder.push_values(entries, |mut builder, entry| {
                builder
                    .push_bind(execution_id)
                    .push_bind(channel_value)
                    .push_bind(entry.entry_index)
                    .push_bind(&entry.entry_json);
            });

            query_builder.push(
                " ON CONFLICT(execution_id, channel, entry_index) DO UPDATE SET entry_json = excluded.entry_json, updated_at = datetime('now', 'subsec')",
            );

            query_builder.build().execute(pool).await?;
            Ok(())
        })
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr, time::Duration};

    use executors::actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    };
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use uuid::Uuid;

    use super::{ExecutionProcessLogEntry, LogEntryRow};
    use crate::models::{
        execution_process::{CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason},
        project::CreateProject,
        session::CreateSession,
        task::CreateTask,
        workspace::CreateWorkspace,
    };
    use utils::log_entries::LogEntryChannel;

    async fn setup_pool() -> Result<(sqlx::SqlitePool, PathBuf), sqlx::Error> {
        let db_path =
            std::env::temp_dir().join(format!("vk-log-entry-test-{}.db", Uuid::new_v4()));
        let db_url = format!("sqlite://{}", db_path.to_string_lossy());
        let options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true)
            .busy_timeout(Duration::from_millis(0));
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok((pool, db_path))
    }

    fn cleanup_db(db_path: PathBuf) {
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    async fn create_execution_process(
        pool: &sqlx::SqlitePool,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        let project_id = Uuid::new_v4();
        let project = CreateProject {
            name: "log-entry-test".to_string(),
            repositories: Vec::new(),
        };
        crate::models::project::Project::create(pool, &project, project_id).await?;

        let task_id = Uuid::new_v4();
        let task = CreateTask {
            project_id,
            title: "log entry task".to_string(),
            description: None,
            status: None,
            parent_workspace_id: None,
            image_ids: None,
            shared_task_id: None,
        };
        crate::models::task::Task::create(pool, &task, task_id).await?;

        let workspace_id = Uuid::new_v4();
        let workspace = CreateWorkspace {
            branch: "main".to_string(),
            agent_working_dir: None,
        };
        crate::models::workspace::Workspace::create(pool, &workspace, workspace_id, task_id)
            .await?;

        let session_id = Uuid::new_v4();
        let session = CreateSession { executor: None };
        crate::models::session::Session::create(pool, &session, session_id, workspace_id).await?;

        let execution_id = Uuid::new_v4();
        let executor_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "echo test".to_string(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: None,
            }),
            None,
        );
        let create_process = CreateExecutionProcess {
            session_id,
            executor_action,
            run_reason: ExecutionProcessRunReason::SetupScript,
        };
        ExecutionProcess::create(pool, &create_process, execution_id, &[]).await?;

        Ok(execution_id)
    }

    #[tokio::test]
    async fn fetch_page_with_cursor_returns_older_entries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (pool, db_path) = setup_pool().await?;
        let execution_id = create_execution_process(&pool).await?;

        for idx in 0..5 {
            let entry_json = format!("{{\"type\":\"STDOUT\",\"content\":\"{idx}\"}}");
            ExecutionProcessLogEntry::upsert_entry(
                &pool,
                execution_id,
                LogEntryChannel::Raw,
                idx,
                &entry_json,
            )
            .await?;
        }

        let first_page =
            ExecutionProcessLogEntry::fetch_page(&pool, execution_id, LogEntryChannel::Raw, 2, None)
                .await?;
        assert_eq!(
            first_page.iter().map(|row| row.entry_index).collect::<Vec<_>>(),
            vec![4, 3]
        );

        let second_page = ExecutionProcessLogEntry::fetch_page(
            &pool,
            execution_id,
            LogEntryChannel::Raw,
            2,
            Some(3),
        )
        .await?;
        assert_eq!(
            second_page
                .iter()
                .map(|row| row.entry_index)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );

        let has_older =
            ExecutionProcessLogEntry::has_older(&pool, execution_id, LogEntryChannel::Raw, 1)
                .await?;
        assert!(has_older);

        let has_older =
            ExecutionProcessLogEntry::has_older(&pool, execution_id, LogEntryChannel::Raw, 0)
                .await?;
        assert!(!has_older);

        cleanup_db(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn stats_report_min_max_and_count() -> Result<(), Box<dyn std::error::Error>> {
        let (pool, db_path) = setup_pool().await?;
        let execution_id = create_execution_process(&pool).await?;

        let entries = vec![
            LogEntryRow {
                entry_index: 2,
                entry_json: "{\"type\":\"STDOUT\",\"content\":\"two\"}".to_string(),
            },
            LogEntryRow {
                entry_index: 4,
                entry_json: "{\"type\":\"STDOUT\",\"content\":\"four\"}".to_string(),
            },
            LogEntryRow {
                entry_index: 3,
                entry_json: "{\"type\":\"STDOUT\",\"content\":\"three\"}".to_string(),
            },
        ];

        ExecutionProcessLogEntry::upsert_entries(
            &pool,
            execution_id,
            LogEntryChannel::Raw,
            &entries,
        )
        .await?;

        let stats = ExecutionProcessLogEntry::stats(&pool, execution_id, LogEntryChannel::Raw)
            .await?
            .unwrap();

        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_index, 2);
        assert_eq!(stats.max_index, 4);

        cleanup_db(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn stats_none_when_empty() -> Result<(), Box<dyn std::error::Error>> {
        let (pool, db_path) = setup_pool().await?;
        let execution_id = create_execution_process(&pool).await?;

        let stats =
            ExecutionProcessLogEntry::stats(&pool, execution_id, LogEntryChannel::Raw).await?;
        assert!(stats.is_none());

        cleanup_db(db_path);
        Ok(())
    }
}
