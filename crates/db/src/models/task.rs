use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use sea_orm::sea_query::{Expr, ExprTrait, JoinType, Order, Query};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
use std::collections::HashMap;

use super::{project::Project, workspace::Workspace};
pub use crate::types::{TaskKind, TaskStatus};

use crate::{
    entities::{execution_process, session, task, task_group, workspace},
    events::{
        EVENT_TASK_CREATED, EVENT_TASK_DELETED, EVENT_TASK_UPDATED, TaskEventPayload,
    },
    models::{event_outbox::EventOutbox, ids},
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub task_kind: TaskKind,
    pub task_group_id: Option<Uuid>,
    pub task_group_node_id: Option<String>,
    pub parent_workspace_id: Option<Uuid>,
    pub shared_task_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskWithAttemptStatus {
    #[serde(flatten)]
    #[ts(flatten)]
    pub task: Task,
    pub has_in_progress_attempt: bool,
    pub last_attempt_failed: bool,
    pub executor: String,
}

impl std::ops::Deref for TaskWithAttemptStatus {
    type Target = Task;
    fn deref(&self) -> &Self::Target {
        &self.task
    }
}

impl std::ops::DerefMut for TaskWithAttemptStatus {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.task
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskRelationships {
    pub parent_task: Option<Task>,
    pub current_workspace: Workspace,
    pub children: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateTask {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub task_kind: Option<TaskKind>,
    pub task_group_id: Option<Uuid>,
    pub task_group_node_id: Option<String>,
    pub parent_workspace_id: Option<Uuid>,
    pub image_ids: Option<Vec<Uuid>>,
    pub shared_task_id: Option<Uuid>,
}

impl CreateTask {
    pub fn from_title_description(
        project_id: Uuid,
        title: String,
        description: Option<String>,
    ) -> Self {
        Self {
            project_id,
            title,
            description,
            status: Some(TaskStatus::Todo),
            task_kind: None,
            task_group_id: None,
            task_group_node_id: None,
            parent_workspace_id: None,
            image_ids: None,
            shared_task_id: None,
        }
    }

    pub fn from_shared_task(
        project_id: Uuid,
        title: String,
        description: Option<String>,
        status: TaskStatus,
        shared_task_id: Uuid,
    ) -> Self {
        Self {
            project_id,
            title,
            description,
            status: Some(status),
            task_kind: None,
            task_group_id: None,
            task_group_node_id: None,
            parent_workspace_id: None,
            image_ids: None,
            shared_task_id: Some(shared_task_id),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub parent_workspace_id: Option<Uuid>,
    pub image_ids: Option<Vec<Uuid>>,
}

impl Task {
    pub fn to_prompt(&self) -> String {
        if let Some(description) = self.description.as_ref().filter(|d| !d.trim().is_empty()) {
            format!("{}\n\n{}", &self.title, description)
        } else {
            self.title.clone()
        }
    }

    async fn from_model<C: ConnectionTrait>(db: &C, model: task::Model) -> Result<Self, DbErr> {
        let project_uuid = ids::project_uuid_by_id(db, model.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let parent_workspace_id = match model.parent_workspace_id {
            Some(id) => ids::workspace_uuid_by_id(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))
                .map(Some)?,
            None => None,
        };
        let shared_task_id = match model.shared_task_id {
            Some(id) => ids::shared_task_uuid_by_id(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Shared task not found".to_string()))
                .map(Some)?,
            None => None,
        };
        let task_group_id = match model.task_group_id {
            Some(id) => ids::task_group_uuid_by_id(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Task group not found".to_string()))
                .map(Some)?,
            None => None,
        };

        Ok(Self {
            id: model.uuid,
            project_id: project_uuid,
            title: model.title,
            description: model.description,
            status: model.status,
            task_kind: model.task_kind,
            task_group_id,
            task_group_node_id: model.task_group_node_id,
            parent_workspace_id,
            shared_task_id,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        })
    }

    async fn attempt_status<C: ConnectionTrait>(
        db: &C,
        task_id: i64,
    ) -> Result<(bool, bool, String), DbErr> {
        let run_reasons = vec![
            crate::types::ExecutionProcessRunReason::SetupScript,
            crate::types::ExecutionProcessRunReason::CleanupScript,
            crate::types::ExecutionProcessRunReason::CodingAgent,
        ];

        let in_progress_query = Query::select()
            .expr(Expr::val(1))
            .from(execution_process::Entity)
            .join(
                JoinType::InnerJoin,
                session::Entity,
                Expr::col((session::Entity, session::Column::Id))
                    .equals((execution_process::Entity, execution_process::Column::SessionId)),
            )
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(Expr::col((workspace::Entity, workspace::Column::TaskId)).eq(task_id))
            .and_where(
                Expr::col((execution_process::Entity, execution_process::Column::Status))
                    .eq(crate::types::ExecutionProcessStatus::Running),
            )
            .and_where(
                Expr::col((execution_process::Entity, execution_process::Column::RunReason))
                    .is_in(run_reasons.clone()),
            )
            .limit(1)
            .to_owned();

        let has_in_progress_attempt = db.query_one(&in_progress_query).await?.is_some();

        let last_status_query = Query::select()
            .column(execution_process::Column::Status)
            .from(execution_process::Entity)
            .join(
                JoinType::InnerJoin,
                session::Entity,
                Expr::col((session::Entity, session::Column::Id))
                    .equals((execution_process::Entity, execution_process::Column::SessionId)),
            )
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(Expr::col((workspace::Entity, workspace::Column::TaskId)).eq(task_id))
            .and_where(
                Expr::col((execution_process::Entity, execution_process::Column::RunReason))
                    .is_in(run_reasons),
            )
            .order_by((execution_process::Entity, execution_process::Column::CreatedAt), Order::Desc)
            .limit(1)
            .to_owned();

        let last_status = db
            .query_one(&last_status_query)
            .await?
            .and_then(|row| row.try_get("", "status").ok());
        let last_attempt_failed = matches!(
            last_status,
            Some(crate::types::ExecutionProcessStatus::Failed)
                | Some(crate::types::ExecutionProcessStatus::Killed)
        );

        let executor_query = Query::select()
            .column(session::Column::Executor)
            .from(session::Entity)
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(Expr::col((workspace::Entity, workspace::Column::TaskId)).eq(task_id))
            .order_by((session::Entity, session::Column::CreatedAt), Order::Desc)
            .limit(1)
            .to_owned();

        let executor = db
            .query_one(&executor_query)
            .await?
            .and_then(|row| row.try_get("", "executor").ok())
            .unwrap_or_default();

        Ok((has_in_progress_attempt, last_attempt_failed, executor))
    }

    pub async fn parent_project<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> Result<Option<Project>, DbErr> {
        Project::find_by_id(db, self.project_id).await
    }

    pub async fn find_by_project_id_with_attempt_status<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<TaskWithAttemptStatus>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let models = task::Entity::find()
            .filter(task::Column::ProjectId.eq(project_row_id))
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            let row_id = model.id;
            let task = Self::from_model(db, model).await?;
            let (has_in_progress_attempt, last_attempt_failed, executor) =
                Self::attempt_status(db, row_id).await?;
            tasks.push(TaskWithAttemptStatus {
                task,
                has_in_progress_attempt,
                last_attempt_failed,
                executor,
            });
        }

        Ok(tasks)
    }

    pub async fn find_all_with_attempt_status<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<TaskWithAttemptStatus>, DbErr> {
        let models = task::Entity::find()
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            let row_id = model.id;
            let task = Self::from_model(db, model).await?;
            let (has_in_progress_attempt, last_attempt_failed, executor) =
                Self::attempt_status(db, row_id).await?;
            tasks.push(TaskWithAttemptStatus {
                task,
                has_in_progress_attempt,
                last_attempt_failed,
                executor,
            });
        }

        Ok(tasks)
    }

    pub async fn find_by_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    pub async fn find_by_shared_task_id<C: ConnectionTrait>(
        db: &C,
        shared_task_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let shared_task_row_id = match ids::shared_task_id_by_uuid(db, shared_task_id).await? {
            Some(id) => id,
            None => return Ok(None),
        };

        let record = task::Entity::find()
            .filter(task::Column::SharedTaskId.eq(shared_task_row_id))
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    pub async fn find_all_shared<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let models = task::Entity::find()
            .filter(task::Column::SharedTaskId.is_not_null())
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::from_model(db, model).await?);
        }
        Ok(tasks)
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateTask,
        task_id: Uuid,
    ) -> Result<Self, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, data.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let parent_workspace_id = match data.parent_workspace_id {
            Some(id) => ids::workspace_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))
                .map(Some)?,
            None => None,
        };
        let shared_task_id = match data.shared_task_id {
            Some(id) => ids::shared_task_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Shared task not found".to_string()))
                .map(Some)?,
            None => None,
        };
        let task_group_id = match data.task_group_id {
            Some(id) => {
                let group_row_id = ids::task_group_id_by_uuid(db, id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Task group not found".to_string()))?;
                if let Some(group) = task_group::Entity::find_by_id(group_row_id)
                    .one(db)
                    .await?
                    && group.project_id != project_row_id
                {
                    return Err(DbErr::Custom(
                        "Task group belongs to a different project".to_string(),
                    ));
                }
                Some(group_row_id)
            }
            None => None,
        };
        let task_group_node_id = data
            .task_group_node_id
            .as_ref()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty());

        let now = Utc::now();
        let active = task::ActiveModel {
            uuid: Set(task_id),
            project_id: Set(project_row_id),
            title: Set(data.title.clone()),
            description: Set(data.description.clone()),
            status: Set(data.status.clone().unwrap_or_default()),
            task_kind: Set(data.task_kind.clone().unwrap_or_default()),
            task_group_id: Set(task_group_id),
            task_group_node_id: Set(task_group_node_id),
            parent_workspace_id: Set(parent_workspace_id),
            shared_task_id: Set(shared_task_id),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        let payload = serde_json::to_value(TaskEventPayload {
            task_id,
            project_id: data.project_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_CREATED, "task", task_id, payload).await?;
        Self::from_model(db, model).await
    }

    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        project_id: Uuid,
        title: String,
        description: Option<String>,
        status: TaskStatus,
        parent_workspace_id: Option<Uuid>,
    ) -> Result<Self, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        if record.project_id != project_row_id {
            return Err(DbErr::RecordNotFound("Task not found".to_string()));
        }

        let status_changed = record.status != status;
        let task_group_id = record.task_group_id;
        let task_kind = record.task_kind.clone();
        let parent_workspace_row_id = match parent_workspace_id {
            Some(id) => ids::workspace_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))
                .map(Some)?,
            None => None,
        };

        let mut active: task::ActiveModel = record.into();
        active.title = Set(title);
        active.description = Set(description);
        active.status = Set(status);
        active.parent_workspace_id = Set(parent_workspace_row_id);
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;
        let payload = serde_json::to_value(TaskEventPayload { task_id: id, project_id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", id, payload).await?;

        if status_changed
            && let Some(task_group_id) = task_group_id
            && task_kind != TaskKind::Group
            && let Err(err) = super::task_group::TaskGroup::sync_entry_task_statuses_by_row_id(
                db,
                task_group_id,
            )
            .await
        {
            tracing::warn!(
                "Failed to sync task group entry status after task update: {}",
                err
            );
        }
        Self::from_model(db, updated).await
    }

    pub async fn update_status<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        status: TaskStatus,
    ) -> Result<(), DbErr> {
        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let project_row_id = record.project_id;
        let task_group_id = record.task_group_id;
        let task_kind = record.task_kind.clone();
        let mut active: task::ActiveModel = record.into();
        active.status = Set(status);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let project_id = ids::project_uuid_by_id(db, project_row_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let payload = serde_json::to_value(TaskEventPayload { task_id: id, project_id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", id, payload).await?;

        if let Some(task_group_id) = task_group_id
            && task_kind != TaskKind::Group
            && let Err(err) = super::task_group::TaskGroup::sync_entry_task_statuses_by_row_id(
                db,
                task_group_id,
            )
            .await
        {
            tracing::warn!("Failed to sync task group entry status: {}", err);
        }
        Ok(())
    }

    pub async fn update_parent_workspace_id<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        parent_workspace_id: Option<Uuid>,
    ) -> Result<(), DbErr> {
        let parent_workspace_row_id = match parent_workspace_id {
            Some(id) => ids::workspace_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))
                .map(Some)?,
            None => None,
        };

        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(task_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let project_row_id = record.project_id;
        let mut active: task::ActiveModel = record.into();
        active.parent_workspace_id = Set(parent_workspace_row_id);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let project_id = ids::project_uuid_by_id(db, project_row_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let payload =
            serde_json::to_value(TaskEventPayload { task_id, project_id })
                .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", task_id, payload).await?;
        Ok(())
    }

    pub async fn nullify_children_by_workspace_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<u64, DbErr> {
        let workspace_row_id = match ids::workspace_id_by_uuid(db, workspace_id).await? {
            Some(id) => id,
            None => return Ok(0),
        };

        let child_tasks = task::Entity::find()
            .filter(task::Column::ParentWorkspaceId.eq(workspace_row_id))
            .all(db)
            .await?;

        let result = task::Entity::update_many()
            .col_expr(task::Column::ParentWorkspaceId, Expr::value(None::<i64>))
            .filter(task::Column::ParentWorkspaceId.eq(workspace_row_id))
            .exec(db)
            .await?;

        for child in child_tasks {
            if let Some(project_id) = ids::project_uuid_by_id(db, child.project_id).await? {
                let payload = serde_json::to_value(TaskEventPayload {
                    task_id: child.uuid,
                    project_id,
                })
                .map_err(|err| DbErr::Custom(err.to_string()))?;
                EventOutbox::enqueue(
                    db,
                    EVENT_TASK_UPDATED,
                    "task",
                    child.uuid,
                    payload,
                )
                .await?;
            }
        }

        Ok(result.rows_affected)
    }

    pub async fn clear_shared_task_ids_for_remote_project<C: ConnectionTrait>(
        db: &C,
        remote_project_id: Uuid,
    ) -> Result<u64, DbErr> {
        let project_ids: Vec<i64> = crate::entities::project::Entity::find()
            .select_only()
            .column(crate::entities::project::Column::Id)
            .filter(crate::entities::project::Column::RemoteProjectId.eq(remote_project_id))
            .into_tuple()
            .all(db)
            .await?;

        if project_ids.is_empty() {
            return Ok(0);
        }

        let tasks = task::Entity::find()
            .filter(task::Column::ProjectId.is_in(project_ids.clone()))
            .all(db)
            .await?;

        let result = task::Entity::update_many()
            .col_expr(task::Column::SharedTaskId, Expr::value(None::<i64>))
            .filter(task::Column::ProjectId.is_in(project_ids))
            .exec(db)
            .await?;

        let mut project_uuid_map: HashMap<i64, Uuid> = HashMap::new();
        for task_model in tasks {
            let project_uuid = if let Some(project_uuid) = project_uuid_map.get(&task_model.project_id)
            {
                *project_uuid
            } else {
                let resolved = ids::project_uuid_by_id(db, task_model.project_id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
                project_uuid_map.insert(task_model.project_id, resolved);
                resolved
            };

            let payload = serde_json::to_value(TaskEventPayload {
                task_id: task_model.uuid,
                project_id: project_uuid,
            })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(
                db,
                EVENT_TASK_UPDATED,
                "task",
                task_model.uuid,
                payload,
            )
            .await?;
        }

        Ok(result.rows_affected)
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, DbErr> {
        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?;

        let Some(record) = record else {
            return Ok(0);
        };

        let project_id = ids::project_uuid_by_id(db, record.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let result = task::Entity::delete_many()
            .filter(task::Column::Uuid.eq(id))
            .exec(db)
            .await?;

        if result.rows_affected > 0 {
            let payload = serde_json::to_value(TaskEventPayload { task_id: id, project_id })
                .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(db, EVENT_TASK_DELETED, "task", id, payload).await?;
        }

        Ok(result.rows_affected)
    }

    pub async fn set_shared_task_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        shared_task_id: Option<Uuid>,
    ) -> Result<(), DbErr> {
        let shared_task_row_id = match shared_task_id {
            Some(id) => ids::shared_task_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Shared task not found".to_string()))
                .map(Some)?,
            None => None,
        };

        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let project_row_id = record.project_id;
        let mut active: task::ActiveModel = record.into();
        active.shared_task_id = Set(shared_task_row_id);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let project_id = ids::project_uuid_by_id(db, project_row_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let payload = serde_json::to_value(TaskEventPayload { task_id: id, project_id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", id, payload).await?;
        Ok(())
    }

    pub async fn batch_unlink_shared_tasks<C: ConnectionTrait>(
        db: &C,
        shared_task_ids: &[Uuid],
    ) -> Result<u64, DbErr> {
        if shared_task_ids.is_empty() {
            return Ok(0);
        }

        let mut ids = Vec::new();
        for shared_task_id in shared_task_ids {
            if let Some(id) = ids::shared_task_id_by_uuid(db, *shared_task_id).await? {
                ids.push(id);
            }
        }

        if ids.is_empty() {
            return Ok(0);
        }

        let tasks = task::Entity::find()
            .filter(task::Column::SharedTaskId.is_in(ids.clone()))
            .all(db)
            .await?;

        let result = task::Entity::update_many()
            .col_expr(task::Column::SharedTaskId, Expr::value(None::<i64>))
            .filter(task::Column::SharedTaskId.is_in(ids))
            .exec(db)
            .await?;

        let mut project_uuid_map: HashMap<i64, Uuid> = HashMap::new();
        for task_model in tasks {
            let project_uuid = if let Some(project_uuid) = project_uuid_map.get(&task_model.project_id)
            {
                *project_uuid
            } else {
                let resolved = ids::project_uuid_by_id(db, task_model.project_id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
                project_uuid_map.insert(task_model.project_id, resolved);
                resolved
            };

            let payload = serde_json::to_value(TaskEventPayload {
                task_id: task_model.uuid,
                project_id: project_uuid,
            })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(
                db,
                EVENT_TASK_UPDATED,
                "task",
                task_model.uuid,
                payload,
            )
            .await?;
        }

        Ok(result.rows_affected)
    }

    pub async fn find_children_by_workspace_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = match ids::workspace_id_by_uuid(db, workspace_id).await? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let models = task::Entity::find()
            .filter(task::Column::ParentWorkspaceId.eq(workspace_row_id))
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::from_model(db, model).await?);
        }
        Ok(tasks)
    }

    pub async fn find_relationships_for_workspace<C: ConnectionTrait>(
        db: &C,
        workspace: &Workspace,
    ) -> Result<TaskRelationships, DbErr> {
        let current_task = Self::find_by_id(db, workspace.task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let parent_task = if let Some(parent_workspace_id) = current_task.parent_workspace_id {
            if let Ok(Some(parent_workspace)) =
                super::workspace::Workspace::find_by_id(db, parent_workspace_id).await
            {
                Self::find_by_id(db, parent_workspace.task_id).await?
            } else {
                None
            }
        } else {
            None
        };

        let children = Self::find_children_by_workspace_id(db, workspace.id).await?;

        Ok(TaskRelationships {
            parent_task,
            current_workspace: workspace.clone(),
            children,
        })
    }
}
