use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("Session not found")]
    NotFound,
    #[error("Workspace not found")]
    WorkspaceNotFound,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Session {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub executor: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateSession {
    pub executor: Option<String>,
}

impl Session {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Session,
            r#"SELECT id AS "id!: Uuid",
                      workspace_id AS "workspace_id!: Uuid",
                      executor,
                      created_at AS "created_at!: DateTime<Utc>",
                      updated_at AS "updated_at!: DateTime<Utc>"
               FROM sessions
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_workspace_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Session,
            r#"SELECT id AS "id!: Uuid",
                      workspace_id AS "workspace_id!: Uuid",
                      executor,
                      created_at AS "created_at!: DateTime<Utc>",
                      updated_at AS "updated_at!: DateTime<Utc>"
               FROM sessions
               WHERE workspace_id = $1
               ORDER BY created_at DESC"#,
            workspace_id
        )
        .fetch_all(pool)
        .await
    }

    /// Find the latest session for a workspace
    pub async fn find_latest_by_workspace_id(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Session,
            r#"SELECT id AS "id!: Uuid",
                      workspace_id AS "workspace_id!: Uuid",
                      executor,
                      created_at AS "created_at!: DateTime<Utc>",
                      updated_at AS "updated_at!: DateTime<Utc>"
               FROM sessions
               WHERE workspace_id = $1
               ORDER BY created_at DESC
               LIMIT 1"#,
            workspace_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_latest_by_workspace_ids(
        pool: &SqlitePool,
        workspace_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Session>, sqlx::Error> {
        if workspace_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            r#"SELECT id,
                      workspace_id,
                      executor,
                      created_at,
                      updated_at
               FROM sessions
               WHERE workspace_id IN ("#,
        );

        let mut separated = query_builder.separated(", ");
        for workspace_id in workspace_ids {
            separated.push_bind(workspace_id);
        }

        query_builder.push(") ORDER BY workspace_id ASC, created_at DESC");

        let sessions: Vec<Session> = query_builder.build_query_as().fetch_all(pool).await?;
        let mut latest_by_workspace = HashMap::new();
        for session in sessions {
            latest_by_workspace
                .entry(session.workspace_id)
                .or_insert(session);
        }

        Ok(latest_by_workspace)
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateSession,
        id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Self, SessionError> {
        Ok(sqlx::query_as!(
            Session,
            r#"INSERT INTO sessions (id, workspace_id, executor)
               VALUES ($1, $2, $3)
               RETURNING id AS "id!: Uuid",
                         workspace_id AS "workspace_id!: Uuid",
                         executor,
                         created_at AS "created_at!: DateTime<Utc>",
                         updated_at AS "updated_at!: DateTime<Utc>""#,
            id,
            workspace_id,
            data.executor
        )
        .fetch_one(pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr, time::Duration};

    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use uuid::Uuid;

    use super::{CreateSession, Session};
    use crate::models::{
        project::CreateProject,
        task::CreateTask,
        workspace::CreateWorkspace,
    };

    async fn setup_pool() -> Result<(sqlx::SqlitePool, PathBuf), sqlx::Error> {
        let db_path =
            std::env::temp_dir().join(format!("vk-session-test-{}.db", Uuid::new_v4()));
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

    async fn create_workspace(
        pool: &sqlx::SqlitePool,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        let project_id = Uuid::new_v4();
        let project = CreateProject {
            name: "session-test".to_string(),
            repositories: Vec::new(),
        };
        crate::models::project::Project::create(pool, &project, project_id).await?;

        let task_id = Uuid::new_v4();
        let task = CreateTask {
            project_id,
            title: "session task".to_string(),
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

        Ok(workspace_id)
    }

    #[tokio::test]
    async fn find_latest_by_workspace_ids_returns_latest() -> Result<(), Box<dyn std::error::Error>>
    {
        let (pool, db_path) = setup_pool().await?;

        let workspace_id = create_workspace(&pool).await?;
        let other_workspace_id = create_workspace(&pool).await?;

        let first = Session::create(
            &pool,
            &CreateSession { executor: None },
            Uuid::new_v4(),
            workspace_id,
        )
        .await?;
        sqlx::query!(
            "UPDATE sessions SET created_at = datetime('now', '-10 minutes') WHERE id = $1",
            first.id
        )
        .execute(&pool)
        .await?;

        let latest = Session::create(
            &pool,
            &CreateSession { executor: Some("codex".to_string()) },
            Uuid::new_v4(),
            workspace_id,
        )
        .await?;

        let other = Session::create(
            &pool,
            &CreateSession { executor: None },
            Uuid::new_v4(),
            other_workspace_id,
        )
        .await?;

        let latest_by_workspace =
            Session::find_latest_by_workspace_ids(&pool, &[workspace_id, other_workspace_id])
                .await?;

        assert_eq!(
            latest_by_workspace.get(&workspace_id).map(|s| s.id),
            Some(latest.id)
        );
        assert_eq!(
            latest_by_workspace
                .get(&other_workspace_id)
                .map(|s| s.id),
            Some(other.id)
        );

        cleanup_db(db_path);
        Ok(())
    }
}
