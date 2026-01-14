use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use super::{
    project::Project,
    task::Task,
    workspace_repo::{RepoWithTargetBranch, WorkspaceRepo},
};
use crate::{
    entities::{session, task, workspace},
    events::{EVENT_WORKSPACE_CREATED, EVENT_WORKSPACE_UPDATED, WorkspaceEventPayload},
    models::{event_outbox::EventOutbox, ids},
};

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Task not found")]
    TaskNotFound,
    #[error("Project not found")]
    ProjectNotFound,
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub workspace_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Workspace {
    pub id: Uuid,
    pub task_id: Uuid,
    pub container_ref: Option<String>,
    pub branch: String,
    pub agent_working_dir: Option<String>,
    pub setup_completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// GitHub PR creation parameters
pub struct CreatePrParams<'a> {
    pub workspace_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub github_token: &'a str,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub base_branch: Option<&'a str>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateFollowUpAttempt {
    pub prompt: String,
}

/// Context data for resume operations (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptResumeContext {
    pub execution_history: String,
    pub cumulative_diffs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub workspace: Workspace,
    pub task: Task,
    pub project: Project,
    pub workspace_repos: Vec<RepoWithTargetBranch>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateWorkspace {
    pub branch: String,
    pub agent_working_dir: Option<String>,
}

impl Workspace {
    fn from_model(model: workspace::Model, task_id: Uuid) -> Self {
        Self {
            id: model.uuid,
            task_id,
            container_ref: model.container_ref,
            branch: model.branch,
            agent_working_dir: model.agent_working_dir,
            setup_completed_at: model.setup_completed_at.map(Into::into),
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub async fn parent_task<C: ConnectionTrait>(&self, db: &C) -> Result<Option<Task>, DbErr> {
        Task::find_by_id(db, self.task_id).await
    }

    /// Fetch all workspaces, optionally filtered by task_id. Newest first.
    pub async fn fetch_all<C: ConnectionTrait>(
        db: &C,
        task_id: Option<Uuid>,
    ) -> Result<Vec<Self>, WorkspaceError> {
        let models = if let Some(task_id) = task_id {
            let task_row_id = ids::task_id_by_uuid(db, task_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
            workspace::Entity::find()
                .filter(workspace::Column::TaskId.eq(task_row_id))
                .order_by_desc(workspace::Column::CreatedAt)
                .all(db)
                .await?
        } else {
            workspace::Entity::find()
                .order_by_desc(workspace::Column::CreatedAt)
                .all(db)
                .await?
        };

        let mut workspaces = Vec::with_capacity(models.len());
        for model in models {
            let task_uuid = ids::task_uuid_by_id(db, model.task_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
            workspaces.push(Self::from_model(model, task_uuid));
        }
        Ok(workspaces)
    }

    pub async fn fetch_all_by_task_ids<C: ConnectionTrait>(
        db: &C,
        task_ids: &[Uuid],
    ) -> Result<Vec<Self>, DbErr> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut row_ids = Vec::new();
        for task_id in task_ids {
            if let Some(id) = ids::task_id_by_uuid(db, *task_id).await? {
                row_ids.push(id);
            }
        }

        if row_ids.is_empty() {
            return Ok(Vec::new());
        }

        let models = workspace::Entity::find()
            .filter(workspace::Column::TaskId.is_in(row_ids))
            .order_by_desc(workspace::Column::CreatedAt)
            .all(db)
            .await?;

        let mut workspaces = Vec::with_capacity(models.len());
        for model in models {
            let task_uuid = ids::task_uuid_by_id(db, model.task_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
            workspaces.push(Self::from_model(model, task_uuid));
        }
        Ok(workspaces)
    }

    pub async fn load_context<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<WorkspaceContext, WorkspaceError> {
        let workspace = Self::find_by_id(db, workspace_id)
            .await?
            .ok_or(WorkspaceError::TaskNotFound)?;

        let task = Task::find_by_id(db, workspace.task_id)
            .await?
            .ok_or(WorkspaceError::TaskNotFound)?;

        let project = Project::find_by_id(db, task.project_id)
            .await?
            .ok_or(WorkspaceError::ProjectNotFound)?;

        let workspace_repos =
            WorkspaceRepo::find_repos_with_target_branch_for_workspace(db, workspace.id).await?;

        Ok(WorkspaceContext {
            workspace,
            task,
            project,
            workspace_repos,
        })
    }

    pub async fn update_container_ref<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        container_ref: &str,
    ) -> Result<(), DbErr> {
        let record = workspace::Entity::find()
            .filter(workspace::Column::Uuid.eq(workspace_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let task_id = ids::task_uuid_by_id(db, record.task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let mut active: workspace::ActiveModel = record.into();
        active.container_ref = Set(Some(container_ref.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let payload = serde_json::to_value(WorkspaceEventPayload {
            workspace_id,
            task_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(
            db,
            EVENT_WORKSPACE_UPDATED,
            "workspace",
            workspace_id,
            payload,
        )
        .await?;
        Ok(())
    }

    pub async fn clear_container_ref<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<(), DbErr> {
        let record = workspace::Entity::find()
            .filter(workspace::Column::Uuid.eq(workspace_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let task_id = ids::task_uuid_by_id(db, record.task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let mut active: workspace::ActiveModel = record.into();
        active.container_ref = Set(None);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let payload = serde_json::to_value(WorkspaceEventPayload {
            workspace_id,
            task_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(
            db,
            EVENT_WORKSPACE_UPDATED,
            "workspace",
            workspace_id,
            payload,
        )
        .await?;
        Ok(())
    }

    pub async fn find_by_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let record = workspace::Entity::find()
            .filter(workspace::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => {
                let task_uuid = ids::task_uuid_by_id(db, model.task_id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
                Ok(Some(Self::from_model(model, task_uuid)))
            }
            None => Ok(None),
        }
    }

    pub async fn container_ref_exists<C: ConnectionTrait>(
        db: &C,
        container_ref: &str,
    ) -> Result<bool, DbErr> {
        Ok(workspace::Entity::find()
            .filter(workspace::Column::ContainerRef.eq(container_ref))
            .one(db)
            .await?
            .is_some())
    }

    /// Find workspaces that are expired (72+ hours since last activity) and eligible for cleanup
    pub async fn find_expired_for_cleanup<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<Workspace>, DbErr> {
        let cutoff = Utc::now() - Duration::hours(72);
        let models = workspace::Entity::find()
            .filter(workspace::Column::ContainerRef.is_not_null())
            .all(db)
            .await?;

        let mut expired = Vec::new();
        for model in models {
            let session_ids: Vec<i64> = session::Entity::find()
                .select_only()
                .column(session::Column::Id)
                .filter(session::Column::WorkspaceId.eq(model.id))
                .into_tuple()
                .all(db)
                .await?;

            if !session_ids.is_empty() {
                let running_exists = crate::entities::execution_process::Entity::find()
                    .filter(crate::entities::execution_process::Column::SessionId.is_in(
                        session_ids.clone(),
                    ))
                    .filter(
                        crate::entities::execution_process::Column::CompletedAt.is_null(),
                    )
                    .one(db)
                    .await?
                    .is_some();

                if running_exists {
                    continue;
                }
            }

            let latest_completed = if session_ids.is_empty() {
                None
            } else {
                crate::entities::execution_process::Entity::find()
                    .filter(crate::entities::execution_process::Column::SessionId.is_in(
                        session_ids,
                    ))
                    .filter(
                        crate::entities::execution_process::Column::CompletedAt.is_not_null(),
                    )
                    .order_by_desc(crate::entities::execution_process::Column::CompletedAt)
                    .one(db)
                    .await?
                    .and_then(|row| row.completed_at)
            };

            let model_updated_at: DateTime<Utc> = model.updated_at.into();
            let last_activity = latest_completed
                .map(DateTime::<Utc>::from)
                .unwrap_or(model_updated_at)
                .max(model_updated_at);

            if last_activity <= cutoff {
                let task_uuid = ids::task_uuid_by_id(db, model.task_id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
                expired.push(Self::from_model(model, task_uuid));
            }
        }

        Ok(expired)
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateWorkspace,
        id: Uuid,
        task_id: Uuid,
    ) -> Result<Self, WorkspaceError> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let now = Utc::now();
        let active = workspace::ActiveModel {
            uuid: Set(id),
            task_id: Set(task_row_id),
            container_ref: Set(None),
            branch: Set(data.branch.clone()),
            agent_working_dir: Set(data.agent_working_dir.clone()),
            setup_completed_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        let payload = serde_json::to_value(WorkspaceEventPayload { workspace_id: id, task_id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_WORKSPACE_CREATED, "workspace", id, payload).await?;
        Ok(Self::from_model(model, task_id))
    }

    pub async fn update_branch_name<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        new_branch_name: &str,
    ) -> Result<(), WorkspaceError> {
        let record = workspace::Entity::find()
            .filter(workspace::Column::Uuid.eq(workspace_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let task_id = ids::task_uuid_by_id(db, record.task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let mut active: workspace::ActiveModel = record.into();
        active.branch = Set(new_branch_name.to_string());
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let payload = serde_json::to_value(WorkspaceEventPayload {
            workspace_id,
            task_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(
            db,
            EVENT_WORKSPACE_UPDATED,
            "workspace",
            workspace_id,
            payload,
        )
        .await?;
        Ok(())
    }

    pub async fn resolve_container_ref<C: ConnectionTrait>(
        db: &C,
        container_ref: &str,
    ) -> Result<ContainerInfo, DbErr> {
        let record = workspace::Entity::find()
            .filter(workspace::Column::ContainerRef.eq(container_ref))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let workspace_id = record.uuid;
        let task_id = ids::task_uuid_by_id(db, record.task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let task_model = task::Entity::find_by_id(record.task_id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let project_id = ids::project_uuid_by_id(db, task_model.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        Ok(ContainerInfo {
            workspace_id,
            task_id,
            project_id,
        })
    }
}
