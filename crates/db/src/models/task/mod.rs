use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
    sea_query::{Alias, Expr, ExprTrait, JoinType, Order, Query},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{project::Project, task_dispatch_state::TaskDispatchState, workspace::Workspace};
pub use crate::types::{TaskKind, TaskStatus};
use crate::{
    entities::{
        execution_process, milestone, project, session, task, task_orchestration_state, workspace,
    },
    events::{EVENT_TASK_CREATED, EVENT_TASK_DELETED, EVENT_TASK_UPDATED, TaskEventPayload},
    models::{event_outbox::EventOutbox, ids},
    types::{
        TaskContinuationStopReasonCode, TaskControlTransferReasonCode, TaskCreatedByKind,
        VkNextAction,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub task_kind: TaskKind,
    pub milestone_id: Option<Uuid>,
    pub milestone_node_id: Option<String>,
    pub parent_workspace_id: Option<Uuid>,
    pub origin_task_id: Option<Uuid>,
    pub created_by_kind: TaskCreatedByKind,
    /// NULL = inherit project default continuation budget.
    pub continuation_turns_override: Option<i32>,
    pub shared_task_id: Option<Uuid>,
    pub archived_kanban_id: Option<Uuid>,
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
    pub dispatch_state: Option<TaskDispatchState>,
    pub orchestration: Option<TaskOrchestrationDiagnostics>,
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
#[serde(rename_all = "snake_case")]
pub enum TaskContinuationBudgetSource {
    ProjectDefault,
    TaskOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskContinuationStopReason {
    pub code: TaskContinuationStopReasonCode,
    pub detail: Option<String>,
    #[ts(type = "Date")]
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskContinuationDiagnostics {
    pub turns_used: i32,
    pub turn_budget: i32,
    pub turns_remaining: i32,
    pub budget_source: TaskContinuationBudgetSource,
    pub last_vk_next_action: Option<VkNextAction>,
    #[ts(type = "Date | null")]
    pub last_vk_next_at: Option<DateTime<Utc>>,
    pub stop_reason: Option<TaskContinuationStopReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskControlTransferDiagnostics {
    pub reason_code: TaskControlTransferReasonCode,
    pub detail: Option<String>,
    #[ts(type = "Date")]
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskOrchestrationDiagnostics {
    pub continuation: TaskContinuationDiagnostics,
    pub last_control_transfer: Option<TaskControlTransferDiagnostics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateTask {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub task_kind: Option<TaskKind>,
    pub milestone_id: Option<Uuid>,
    pub milestone_node_id: Option<String>,
    pub parent_workspace_id: Option<Uuid>,
    pub origin_task_id: Option<Uuid>,
    pub created_by_kind: Option<TaskCreatedByKind>,
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
            milestone_id: None,
            milestone_node_id: None,
            parent_workspace_id: None,
            origin_task_id: None,
            created_by_kind: None,
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
            milestone_id: None,
            milestone_node_id: None,
            parent_workspace_id: None,
            origin_task_id: None,
            created_by_kind: None,
            image_ids: None,
            shared_task_id: Some(shared_task_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskLineageSummary {
    pub origin_task: Option<Task>,
    pub follow_up_tasks: Vec<Task>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub parent_workspace_id: Option<Uuid>,
    pub image_ids: Option<Vec<Uuid>>,
    #[serde(deserialize_with = "deserialize_optional_i32_as_double_option")]
    pub continuation_turns_override: Option<Option<i32>>,
}

#[derive(Debug, Clone)]
pub struct TaskUpdateParams {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub parent_workspace_id: Option<Uuid>,
    pub continuation_turns_override: Option<Option<i32>>,
}

fn deserialize_optional_i32_as_double_option<'de, D>(
    deserializer: D,
) -> Result<Option<Option<i32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<i32>::deserialize(deserializer)?))
}

impl Task {
    fn archived_task_write_error() -> DbErr {
        DbErr::Custom("Task is archived. Restore it before modifying.".to_string())
    }

    /// Auto-managed tasks are milestone node tasks inside auto milestones.
    pub async fn is_auto_managed<C: ConnectionTrait>(&self, db: &C) -> Result<bool, DbErr> {
        let milestone_id = match self.milestone_id {
            Some(id) => id,
            None => return Ok(false),
        };

        if self.task_kind == TaskKind::Milestone {
            return Ok(false);
        }
        if self
            .milestone_node_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .is_none()
        {
            return Ok(false);
        }

        let milestone_record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(milestone_id))
            .one(db)
            .await?;
        let Some(milestone_record) = milestone_record else {
            return Ok(false);
        };

        Ok(milestone_record.automation_mode == crate::types::MilestoneAutomationMode::Auto)
    }

    async fn resolve_task_orchestration_diagnostics<C: ConnectionTrait>(
        db: &C,
        task_row_id: i64,
        task: &Task,
    ) -> Result<Option<TaskOrchestrationDiagnostics>, DbErr> {
        let milestone_id = match task.milestone_id {
            Some(id) => id,
            None => return Ok(None),
        };

        // Only milestone node tasks in auto milestones are considered "auto-managed".
        if task.task_kind == TaskKind::Milestone {
            return Ok(None);
        }
        if task
            .milestone_node_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .is_none()
        {
            return Ok(None);
        }

        let milestone_record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(milestone_id))
            .one(db)
            .await?;
        let Some(milestone_record) = milestone_record else {
            return Ok(None);
        };
        if milestone_record.automation_mode != crate::types::MilestoneAutomationMode::Auto {
            return Ok(None);
        }

        let project_record = project::Entity::find()
            .filter(project::Column::Uuid.eq(task.project_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let effective_budget = std::cmp::max(
            task.continuation_turns_override
                .unwrap_or(project_record.default_continuation_turns),
            0,
        );
        let budget_source = if task.continuation_turns_override.is_some() {
            TaskContinuationBudgetSource::TaskOverride
        } else {
            TaskContinuationBudgetSource::ProjectDefault
        };

        let state = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let turns_used = std::cmp::max(
            state
                .as_ref()
                .map(|state| state.continuation_turns_used)
                .unwrap_or(0),
            0,
        );
        let turns_remaining = std::cmp::max(effective_budget - turns_used, 0);

        let stop_reason = state.as_ref().and_then(|state| {
            let code = state.last_continuation_stop_reason_code.clone()?;
            let at = state.last_continuation_stop_at.map(Into::into)?;
            Some(TaskContinuationStopReason {
                code,
                detail: state.last_continuation_stop_reason_detail.clone(),
                at,
            })
        });

        let last_control_transfer = state.as_ref().and_then(|state| {
            let reason_code = state.last_control_transfer_reason_code.clone()?;
            let at = state.last_control_transfer_at.map(Into::into)?;
            Some(TaskControlTransferDiagnostics {
                reason_code,
                detail: state.last_control_transfer_detail.clone(),
                at,
            })
        });

        Ok(Some(TaskOrchestrationDiagnostics {
            continuation: TaskContinuationDiagnostics {
                turns_used,
                turn_budget: effective_budget,
                turns_remaining,
                budget_source,
                last_vk_next_action: state
                    .as_ref()
                    .and_then(|state| state.last_vk_next_action.clone()),
                last_vk_next_at: state
                    .as_ref()
                    .and_then(|state| state.last_vk_next_at.map(Into::into)),
                stop_reason,
            },
            last_control_transfer,
        }))
    }

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
        let origin_task_id = match model.origin_task_id {
            Some(id) => ids::task_uuid_by_id(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Origin task not found".to_string()))
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
        let archived_kanban_id = match model.archived_kanban_id {
            Some(id) => ids::archived_kanban_uuid_by_id(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound(
                    "Archived kanban not found".to_string(),
                ))
                .map(Some)?,
            None => None,
        };
        let milestone_id = match model.milestone_id {
            Some(id) => ids::milestone_uuid_by_id(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))
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
            milestone_id,
            milestone_node_id: model.milestone_node_id,
            parent_workspace_id,
            origin_task_id,
            created_by_kind: model.created_by_kind,
            continuation_turns_override: model.continuation_turns_override,
            shared_task_id,
            archived_kanban_id,
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
                Expr::col((session::Entity, session::Column::Id)).equals((
                    execution_process::Entity,
                    execution_process::Column::SessionId,
                )),
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
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::RunReason,
                ))
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
                Expr::col((session::Entity, session::Column::Id)).equals((
                    execution_process::Entity,
                    execution_process::Column::SessionId,
                )),
            )
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(Expr::col((workspace::Entity, workspace::Column::TaskId)).eq(task_id))
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::RunReason,
                ))
                .is_in(run_reasons),
            )
            .order_by(
                (
                    execution_process::Entity,
                    execution_process::Column::CreatedAt,
                ),
                Order::Desc,
            )
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

    async fn with_attempt_status<C: ConnectionTrait>(
        db: &C,
        model: task::Model,
    ) -> Result<TaskWithAttemptStatus, DbErr> {
        let row_id = model.id;
        let task = Self::from_model(db, model).await?;
        let (has_in_progress_attempt, last_attempt_failed, executor) =
            Self::attempt_status(db, row_id).await?;
        let dispatch_state = TaskDispatchState::find_by_task_id(db, task.id).await?;

        let orchestration = Self::resolve_task_orchestration_diagnostics(db, row_id, &task).await?;

        let task_with_status = TaskWithAttemptStatus {
            task,
            has_in_progress_attempt,
            last_attempt_failed,
            executor,
            dispatch_state,
            orchestration,
        };
        Ok(task_with_status)
    }

    pub async fn has_running_attempts<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
    ) -> Result<bool, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
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
                Expr::col((session::Entity, session::Column::Id)).equals((
                    execution_process::Entity,
                    execution_process::Column::SessionId,
                )),
            )
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(Expr::col((workspace::Entity, workspace::Column::TaskId)).eq(task_row_id))
            .and_where(
                Expr::col((execution_process::Entity, execution_process::Column::Status))
                    .eq(crate::types::ExecutionProcessStatus::Running),
            )
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::RunReason,
                ))
                .is_in(run_reasons),
            )
            .limit(1)
            .to_owned();

        Ok(db.query_one(&in_progress_query).await?.is_some())
    }

    pub async fn latest_attempt_workspace_id<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
    ) -> Result<Option<Uuid>, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let run_reasons = vec![
            crate::types::ExecutionProcessRunReason::SetupScript,
            crate::types::ExecutionProcessRunReason::CleanupScript,
            crate::types::ExecutionProcessRunReason::CodingAgent,
        ];

        let latest_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::Uuid)),
                Alias::new("workspace_uuid"),
            )
            .from(execution_process::Entity)
            .join(
                JoinType::InnerJoin,
                session::Entity,
                Expr::col((session::Entity, session::Column::Id)).equals((
                    execution_process::Entity,
                    execution_process::Column::SessionId,
                )),
            )
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(Expr::col((workspace::Entity, workspace::Column::TaskId)).eq(task_row_id))
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::RunReason,
                ))
                .is_in(run_reasons),
            )
            .order_by(
                (
                    execution_process::Entity,
                    execution_process::Column::CreatedAt,
                ),
                Order::Desc,
            )
            .limit(1)
            .to_owned();

        let latest_workspace_id = db
            .query_one(&latest_query)
            .await?
            .and_then(|row| row.try_get("", "workspace_uuid").ok());

        Ok(latest_workspace_id)
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
            .filter(task::Column::ArchivedKanbanId.is_null())
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::with_attempt_status(db, model).await?);
        }

        Ok(tasks)
    }

    pub async fn find_by_milestone_id<C: ConnectionTrait>(
        db: &C,
        milestone_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let milestone_row_id = ids::milestone_id_by_uuid(db, milestone_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))?;

        let models = task::Entity::find()
            .filter(task::Column::MilestoneId.eq(milestone_row_id))
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::from_model(db, model).await?);
        }
        Ok(tasks)
    }

    pub async fn find_all_with_attempt_status<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<TaskWithAttemptStatus>, DbErr> {
        let models = task::Entity::find()
            .filter(task::Column::ArchivedKanbanId.is_null())
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::with_attempt_status(db, model).await?);
        }

        Ok(tasks)
    }

    pub async fn find_filtered_with_attempt_status<C: ConnectionTrait>(
        db: &C,
        project_id: Option<Uuid>,
        include_archived: bool,
        archived_kanban_id: Option<Uuid>,
    ) -> Result<Vec<TaskWithAttemptStatus>, DbErr> {
        let project_row_id = match project_id {
            Some(project_id) => ids::project_id_by_uuid(db, project_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Project not found".to_string()))
                .map(Some)?,
            None => None,
        };

        let archived_kanban_row_id = match archived_kanban_id {
            Some(archived_kanban_id) => {
                let row_id = ids::archived_kanban_id_by_uuid(db, archived_kanban_id).await?;
                match row_id {
                    Some(row_id) => Some(row_id),
                    None => return Ok(Vec::new()),
                }
            }
            None => None,
        };

        let mut query = task::Entity::find().order_by_desc(task::Column::CreatedAt);

        if let Some(project_row_id) = project_row_id {
            query = query.filter(task::Column::ProjectId.eq(project_row_id));
        }

        if let Some(archived_kanban_row_id) = archived_kanban_row_id {
            query = query.filter(task::Column::ArchivedKanbanId.eq(archived_kanban_row_id));
        } else if !include_archived {
            query = query.filter(task::Column::ArchivedKanbanId.is_null());
        }

        let models = query.all(db).await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::with_attempt_status(db, model).await?);
        }

        Ok(tasks)
    }

    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    pub async fn find_by_id_with_attempt_status<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<TaskWithAttemptStatus>, DbErr> {
        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::with_attempt_status(db, model).await?)),
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

    pub async fn find_by_origin_task_id<C: ConnectionTrait>(
        db: &C,
        origin_task_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let origin_task_row_id = match ids::task_id_by_uuid(db, origin_task_id).await? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let models = task::Entity::find()
            .filter(task::Column::OriginTaskId.eq(origin_task_row_id))
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            tasks.push(Self::from_model(db, model).await?);
        }
        Ok(tasks)
    }

    pub async fn lineage_summary<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
    ) -> Result<TaskLineageSummary, DbErr> {
        let current_task = Self::find_by_id(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let origin_task = match current_task.origin_task_id {
            Some(origin_task_id) => Self::find_by_id(db, origin_task_id).await?,
            None => None,
        };
        let follow_up_tasks = Self::find_by_origin_task_id(db, current_task.id).await?;

        Ok(TaskLineageSummary {
            origin_task,
            follow_up_tasks,
        })
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
        let origin_task_id = match data.origin_task_id {
            Some(id) => ids::task_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound("Origin task not found".to_string()))
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
        let milestone_id = match data.milestone_id {
            Some(id) => {
                let milestone_row_id = ids::milestone_id_by_uuid(db, id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))?;
                if let Some(milestone) = milestone::Entity::find_by_id(milestone_row_id)
                    .one(db)
                    .await?
                    && milestone.project_id != project_row_id
                {
                    return Err(DbErr::Custom(
                        "Milestone belongs to a different project".to_string(),
                    ));
                }
                Some(milestone_row_id)
            }
            None => None,
        };
        let milestone_node_id = data
            .milestone_node_id
            .as_ref()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty());
        let task_kind = data.task_kind.clone().unwrap_or_default();
        if task_kind == TaskKind::Milestone {
            if milestone_id.is_none() {
                return Err(DbErr::Custom(
                    "Milestone entry task requires milestone_id".to_string(),
                ));
            }
            if milestone_node_id.is_some() {
                return Err(DbErr::Custom(
                    "Milestone entry task cannot set milestone_node_id".to_string(),
                ));
            }
        }

        let now = Utc::now();
        let active = task::ActiveModel {
            uuid: Set(task_id),
            project_id: Set(project_row_id),
            title: Set(data.title.clone()),
            description: Set(data.description.clone()),
            status: Set(data.status.clone().unwrap_or_default()),
            task_kind: Set(task_kind.clone()),
            milestone_id: Set(milestone_id),
            milestone_node_id: Set(milestone_node_id),
            parent_workspace_id: Set(parent_workspace_id),
            origin_task_id: Set(origin_task_id),
            created_by_kind: Set(data.created_by_kind.clone().unwrap_or_default()),
            continuation_turns_override: Set(None),
            shared_task_id: Set(shared_task_id),
            archived_kanban_id: Set(None),
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
        params: TaskUpdateParams,
    ) -> Result<Self, DbErr> {
        let TaskUpdateParams {
            project_id,
            title,
            description,
            status,
            parent_workspace_id,
            continuation_turns_override,
        } = params;
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

        if record.archived_kanban_id.is_some() {
            return Err(Self::archived_task_write_error());
        }

        let status_changed = record.status != status;
        let milestone_id = record.milestone_id;
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
        active.status = Set(status.clone());
        active.parent_workspace_id = Set(parent_workspace_row_id);
        if let Some(value) = continuation_turns_override {
            active.continuation_turns_override = Set(value.map(|turns| std::cmp::max(turns, 0)));
        }
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: id,
            project_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", id, payload).await?;

        if status_changed {
            if task_kind == TaskKind::Milestone {
                if let Some(milestone_id) = milestone_id {
                    let milestone_record = milestone::Entity::find_by_id(milestone_id)
                        .one(db)
                        .await?
                        .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))?;
                    if milestone_record.status != status {
                        let mut milestone_active: milestone::ActiveModel = milestone_record.into();
                        milestone_active.status = Set(status.clone());
                        milestone_active.updated_at = Set(Utc::now().into());
                        milestone_active.update(db).await?;
                    }
                }
            } else if let Some(milestone_id) = milestone_id
                && let Err(err) = super::milestone::Milestone::sync_entry_task_statuses_by_row_id(
                    db,
                    milestone_id,
                )
                .await
            {
                tracing::warn!(
                    "Failed to sync milestone entry status after task update: {}",
                    err
                );
            }
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

        if record.archived_kanban_id.is_some() {
            return Err(Self::archived_task_write_error());
        }

        let project_row_id = record.project_id;
        let milestone_id = record.milestone_id;
        let task_kind = record.task_kind.clone();
        let status_changed = record.status != status;
        let mut active: task::ActiveModel = record.into();
        active.status = Set(status.clone());
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let project_id = ids::project_uuid_by_id(db, project_row_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: id,
            project_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", id, payload).await?;

        if status_changed {
            if task_kind == TaskKind::Milestone {
                if let Some(milestone_id) = milestone_id {
                    let milestone_record = milestone::Entity::find_by_id(milestone_id)
                        .one(db)
                        .await?
                        .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))?;
                    if milestone_record.status != status {
                        let mut milestone_active: milestone::ActiveModel = milestone_record.into();
                        milestone_active.status = Set(status.clone());
                        milestone_active.updated_at = Set(Utc::now().into());
                        milestone_active.update(db).await?;
                    }
                }
            } else if let Some(milestone_id) = milestone_id
                && let Err(err) = super::milestone::Milestone::sync_entry_task_statuses_by_row_id(
                    db,
                    milestone_id,
                )
                .await
            {
                tracing::warn!("Failed to sync milestone entry status: {}", err);
            }
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

        if record.archived_kanban_id.is_some() {
            return Err(Self::archived_task_write_error());
        }

        let project_row_id = record.project_id;
        let mut active: task::ActiveModel = record.into();
        active.parent_workspace_id = Set(parent_workspace_row_id);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let project_id = ids::project_uuid_by_id(db, project_row_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let payload = serde_json::to_value(TaskEventPayload {
            task_id,
            project_id,
        })
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
                EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", child.uuid, payload).await?;
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
            let project_uuid =
                if let Some(project_uuid) = project_uuid_map.get(&task_model.project_id) {
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
            EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", task_model.uuid, payload).await?;
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

        if record.archived_kanban_id.is_some() {
            return Err(DbErr::Custom(
                "Task is archived. Delete its archive to remove it.".to_string(),
            ));
        }

        Self::delete_allow_archived(db, id).await
    }

    pub async fn delete_allow_archived<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, DbErr> {
        let record = task::Entity::find()
            .filter(task::Column::Uuid.eq(id))
            .one(db)
            .await?;

        let Some(record) = record else {
            return Ok(0);
        };

        let task_id = record.uuid;
        let project_id = ids::project_uuid_by_id(db, record.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let result = task::Entity::delete_many()
            .filter(task::Column::Uuid.eq(task_id))
            .exec(db)
            .await?;

        if result.rows_affected > 0 {
            let payload = serde_json::to_value(TaskEventPayload {
                task_id,
                project_id,
            })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(db, EVENT_TASK_DELETED, "task", task_id, payload).await?;
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

        if record.archived_kanban_id.is_some() {
            return Err(Self::archived_task_write_error());
        }

        let project_row_id = record.project_id;
        let mut active: task::ActiveModel = record.into();
        active.shared_task_id = Set(shared_task_row_id);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let project_id = ids::project_uuid_by_id(db, project_row_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: id,
            project_id,
        })
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
            let project_uuid =
                if let Some(project_uuid) = project_uuid_map.get(&task_model.project_id) {
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
            EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", task_model.uuid, payload).await?;
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
