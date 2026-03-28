use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
    sea_query::{Alias, Condition, Expr, ExprTrait, JoinType, Order, Query},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::{project::Project, task_dispatch_state::TaskDispatchState, workspace::Workspace};
pub use crate::types::{TaskKind, TaskStatus};
use crate::{
    entities::{
        archived_kanban, execution_process, milestone, project, session, shared_task, task,
        task_dispatch_state, task_orchestration_state, workspace,
    },
    events::{EVENT_TASK_CREATED, EVENT_TASK_DELETED, EVENT_TASK_UPDATED, TaskEventPayload},
    models::{event_outbox::EventOutbox, ids},
    types::{
        MilestoneAutomationMode, TaskContinuationStopReasonCode, TaskControlTransferReasonCode,
        TaskCreatedByKind, VkNextAction,
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

    async fn attempt_status_bulk<C: ConnectionTrait>(
        db: &C,
        task_row_ids: &[i64],
    ) -> Result<HashMap<i64, (bool, bool, String)>, DbErr> {
        if task_row_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let task_row_ids = task_row_ids.to_vec();
        let run_reasons = vec![
            crate::types::ExecutionProcessRunReason::SetupScript,
            crate::types::ExecutionProcessRunReason::CleanupScript,
            crate::types::ExecutionProcessRunReason::CodingAgent,
        ];

        let in_progress_query = Query::select()
            .distinct()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
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
            .and_where(
                Expr::col((workspace::Entity, workspace::Column::TaskId))
                    .is_in(task_row_ids.clone()),
            )
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
            .to_owned();

        let mut in_progress = HashSet::new();
        for row in db.query_all(&in_progress_query).await? {
            if let Ok(task_row_id) = row.try_get::<i64>("", "task_id") {
                in_progress.insert(task_row_id);
            }
        }

        // Latest execution_process status per task (stable tie-break by created_at then id).
        let latest_status_time_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
            )
            .expr_as(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::CreatedAt,
                ))
                .max(),
                Alias::new("max_created_at"),
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
            .and_where(
                Expr::col((workspace::Entity, workspace::Column::TaskId))
                    .is_in(task_row_ids.clone()),
            )
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::RunReason,
                ))
                .is_in(run_reasons.clone()),
            )
            .group_by_columns([(workspace::Entity, workspace::Column::TaskId)])
            .to_owned();

        let latest_status_id_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
            )
            .expr_as(
                Expr::col((execution_process::Entity, execution_process::Column::Id)).max(),
                Alias::new("max_id"),
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
            .join_subquery(
                JoinType::InnerJoin,
                latest_status_time_query,
                Alias::new("latest_time"),
                Condition::all()
                    .add(
                        Expr::col((workspace::Entity, workspace::Column::TaskId))
                            .equals((Alias::new("latest_time"), Alias::new("task_id"))),
                    )
                    .add(
                        Expr::col((
                            execution_process::Entity,
                            execution_process::Column::CreatedAt,
                        ))
                        .equals((Alias::new("latest_time"), Alias::new("max_created_at"))),
                    ),
            )
            .and_where(
                Expr::col((workspace::Entity, workspace::Column::TaskId))
                    .is_in(task_row_ids.clone()),
            )
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::RunReason,
                ))
                .is_in(run_reasons),
            )
            .group_by_columns([(workspace::Entity, workspace::Column::TaskId)])
            .to_owned();

        let latest_status_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
            )
            .expr_as(
                Expr::col((execution_process::Entity, execution_process::Column::Status)),
                Alias::new("status"),
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
            .join_subquery(
                JoinType::InnerJoin,
                latest_status_id_query,
                Alias::new("latest_status"),
                Condition::all()
                    .add(
                        Expr::col((workspace::Entity, workspace::Column::TaskId))
                            .equals((Alias::new("latest_status"), Alias::new("task_id"))),
                    )
                    .add(
                        Expr::col((execution_process::Entity, execution_process::Column::Id))
                            .equals((Alias::new("latest_status"), Alias::new("max_id"))),
                    ),
            )
            .to_owned();

        let mut last_status_by_task_row_id = HashMap::new();
        for row in db.query_all(&latest_status_query).await? {
            let task_row_id = row.try_get::<i64>("", "task_id")?;
            let status = row.try_get::<crate::types::ExecutionProcessStatus>("", "status")?;
            last_status_by_task_row_id
                .entry(task_row_id)
                .or_insert(status);
        }

        // Latest session executor per task (stable tie-break by created_at then id).
        let latest_executor_time_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
            )
            .expr_as(
                Expr::col((session::Entity, session::Column::CreatedAt)).max(),
                Alias::new("max_created_at"),
            )
            .from(session::Entity)
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .and_where(
                Expr::col((workspace::Entity, workspace::Column::TaskId))
                    .is_in(task_row_ids.clone()),
            )
            .group_by_columns([(workspace::Entity, workspace::Column::TaskId)])
            .to_owned();

        let latest_executor_id_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
            )
            .expr_as(
                Expr::col((session::Entity, session::Column::Id)).max(),
                Alias::new("max_id"),
            )
            .from(session::Entity)
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .join_subquery(
                JoinType::InnerJoin,
                latest_executor_time_query,
                Alias::new("latest_time"),
                Condition::all()
                    .add(
                        Expr::col((workspace::Entity, workspace::Column::TaskId))
                            .equals((Alias::new("latest_time"), Alias::new("task_id"))),
                    )
                    .add(
                        Expr::col((session::Entity, session::Column::CreatedAt))
                            .equals((Alias::new("latest_time"), Alias::new("max_created_at"))),
                    ),
            )
            .and_where(
                Expr::col((workspace::Entity, workspace::Column::TaskId))
                    .is_in(task_row_ids.clone()),
            )
            .group_by_columns([(workspace::Entity, workspace::Column::TaskId)])
            .to_owned();

        let latest_executor_query = Query::select()
            .expr_as(
                Expr::col((workspace::Entity, workspace::Column::TaskId)),
                Alias::new("task_id"),
            )
            .expr_as(
                Expr::col((session::Entity, session::Column::Executor)),
                Alias::new("executor"),
            )
            .from(session::Entity)
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::Id))
                    .equals((session::Entity, session::Column::WorkspaceId)),
            )
            .join_subquery(
                JoinType::InnerJoin,
                latest_executor_id_query,
                Alias::new("latest_session"),
                Condition::all()
                    .add(
                        Expr::col((workspace::Entity, workspace::Column::TaskId))
                            .equals((Alias::new("latest_session"), Alias::new("task_id"))),
                    )
                    .add(
                        Expr::col((session::Entity, session::Column::Id))
                            .equals((Alias::new("latest_session"), Alias::new("max_id"))),
                    ),
            )
            .to_owned();

        let mut executor_by_task_row_id = HashMap::new();
        for row in db.query_all(&latest_executor_query).await? {
            let task_row_id = row.try_get::<i64>("", "task_id")?;
            let executor = row
                .try_get::<String>("", "executor")
                .ok()
                .unwrap_or_default();
            executor_by_task_row_id
                .entry(task_row_id)
                .or_insert(executor);
        }

        let mut results = HashMap::with_capacity(task_row_ids.len());
        for task_row_id in &task_row_ids {
            let has_in_progress_attempt = in_progress.contains(task_row_id);
            let last_attempt_failed = matches!(
                last_status_by_task_row_id.get(task_row_id),
                Some(crate::types::ExecutionProcessStatus::Failed)
                    | Some(crate::types::ExecutionProcessStatus::Killed)
            );
            let executor = executor_by_task_row_id
                .get(task_row_id)
                .cloned()
                .unwrap_or_default();
            results.insert(
                *task_row_id,
                (has_in_progress_attempt, last_attempt_failed, executor),
            );
        }

        Ok(results)
    }

    async fn with_attempt_status_bulk<C: ConnectionTrait>(
        db: &C,
        models: Vec<task::Model>,
    ) -> Result<Vec<TaskWithAttemptStatus>, DbErr> {
        if models.is_empty() {
            return Ok(Vec::new());
        }

        let mut task_row_ids = Vec::with_capacity(models.len());
        let mut task_uuid_by_row_id = HashMap::with_capacity(models.len());
        let mut project_row_ids = Vec::with_capacity(models.len());

        let mut parent_workspace_row_ids = Vec::new();
        let mut origin_task_row_ids = Vec::new();
        let mut shared_task_row_ids = Vec::new();
        let mut archived_kanban_row_ids = Vec::new();
        let mut milestone_row_ids = Vec::new();

        for model in &models {
            task_row_ids.push(model.id);
            task_uuid_by_row_id.insert(model.id, model.uuid);
            project_row_ids.push(model.project_id);
            if let Some(id) = model.parent_workspace_id {
                parent_workspace_row_ids.push(id);
            }
            if let Some(id) = model.origin_task_id {
                origin_task_row_ids.push(id);
            }
            if let Some(id) = model.shared_task_id {
                shared_task_row_ids.push(id);
            }
            if let Some(id) = model.archived_kanban_id {
                archived_kanban_row_ids.push(id);
            }
            if let Some(id) = model.milestone_id {
                milestone_row_ids.push(id);
            }
        }

        project_row_ids.sort_unstable();
        project_row_ids.dedup();
        parent_workspace_row_ids.sort_unstable();
        parent_workspace_row_ids.dedup();
        origin_task_row_ids.sort_unstable();
        origin_task_row_ids.dedup();
        shared_task_row_ids.sort_unstable();
        shared_task_row_ids.dedup();
        archived_kanban_row_ids.sort_unstable();
        archived_kanban_row_ids.dedup();
        milestone_row_ids.sort_unstable();
        milestone_row_ids.dedup();

        let project_rows: Vec<(i64, Uuid, i32)> = project::Entity::find()
            .select_only()
            .column(project::Column::Id)
            .column(project::Column::Uuid)
            .column(project::Column::DefaultContinuationTurns)
            .filter(project::Column::Id.is_in(project_row_ids.clone()))
            .into_tuple()
            .all(db)
            .await?;

        let mut project_uuid_by_row_id = HashMap::with_capacity(project_rows.len());
        let mut project_default_continuation_turns_by_row_id =
            HashMap::with_capacity(project_rows.len());
        for (row_id, uuid, default_turns) in project_rows {
            project_uuid_by_row_id.insert(row_id, uuid);
            project_default_continuation_turns_by_row_id.insert(row_id, default_turns);
        }

        let workspace_uuid_by_row_id: HashMap<i64, Uuid> = if parent_workspace_row_ids.is_empty() {
            HashMap::new()
        } else {
            workspace::Entity::find()
                .select_only()
                .column(workspace::Column::Id)
                .column(workspace::Column::Uuid)
                .filter(workspace::Column::Id.is_in(parent_workspace_row_ids))
                .into_tuple::<(i64, Uuid)>()
                .all(db)
                .await?
                .into_iter()
                .collect()
        };

        let origin_task_uuid_by_row_id: HashMap<i64, Uuid> = if origin_task_row_ids.is_empty() {
            HashMap::new()
        } else {
            task::Entity::find()
                .select_only()
                .column(task::Column::Id)
                .column(task::Column::Uuid)
                .filter(task::Column::Id.is_in(origin_task_row_ids))
                .into_tuple::<(i64, Uuid)>()
                .all(db)
                .await?
                .into_iter()
                .collect()
        };

        let shared_task_uuid_by_row_id: HashMap<i64, Uuid> = if shared_task_row_ids.is_empty() {
            HashMap::new()
        } else {
            shared_task::Entity::find()
                .select_only()
                .column(shared_task::Column::Id)
                .column(shared_task::Column::Uuid)
                .filter(shared_task::Column::Id.is_in(shared_task_row_ids))
                .into_tuple::<(i64, Uuid)>()
                .all(db)
                .await?
                .into_iter()
                .collect()
        };

        let archived_kanban_uuid_by_row_id: HashMap<i64, Uuid> =
            if archived_kanban_row_ids.is_empty() {
                HashMap::new()
            } else {
                archived_kanban::Entity::find()
                    .select_only()
                    .column(archived_kanban::Column::Id)
                    .column(archived_kanban::Column::Uuid)
                    .filter(archived_kanban::Column::Id.is_in(archived_kanban_row_ids))
                    .into_tuple::<(i64, Uuid)>()
                    .all(db)
                    .await?
                    .into_iter()
                    .collect()
            };

        let milestone_rows: Vec<(i64, Uuid, MilestoneAutomationMode)> =
            if milestone_row_ids.is_empty() {
                Vec::new()
            } else {
                milestone::Entity::find()
                    .select_only()
                    .column(milestone::Column::Id)
                    .column(milestone::Column::Uuid)
                    .column(milestone::Column::AutomationMode)
                    .filter(milestone::Column::Id.is_in(milestone_row_ids))
                    .into_tuple()
                    .all(db)
                    .await?
            };

        let mut milestone_uuid_by_row_id = HashMap::with_capacity(milestone_rows.len());
        let mut milestone_automation_mode_by_row_id = HashMap::with_capacity(milestone_rows.len());
        for (row_id, uuid, mode) in milestone_rows {
            milestone_uuid_by_row_id.insert(row_id, uuid);
            milestone_automation_mode_by_row_id.insert(row_id, mode);
        }

        let attempt_status_by_task_row_id = Self::attempt_status_bulk(db, &task_row_ids).await?;

        let dispatch_state_models = task_dispatch_state::Entity::find()
            .filter(task_dispatch_state::Column::TaskId.is_in(task_row_ids.clone()))
            .all(db)
            .await?;

        let mut dispatch_state_by_task_row_id = HashMap::with_capacity(dispatch_state_models.len());
        for model in dispatch_state_models {
            let Some(task_uuid) = task_uuid_by_row_id.get(&model.task_id).copied() else {
                continue;
            };
            dispatch_state_by_task_row_id.insert(
                model.task_id,
                TaskDispatchState {
                    task_id: task_uuid,
                    controller: model.controller,
                    status: model.status,
                    retry_count: model.retry_count,
                    max_retries: model.max_retries,
                    last_error: model.last_error,
                    blocked_reason: model.blocked_reason,
                    next_retry_at: model.next_retry_at.map(Into::into),
                    claim_expires_at: model.claim_expires_at.map(Into::into),
                    created_at: model.created_at.into(),
                    updated_at: model.updated_at.into(),
                },
            );
        }

        let mut orchestration_state_by_task_row_id = HashMap::new();
        let candidate_task_row_ids: Vec<i64> = models
            .iter()
            .filter(|model| model.milestone_id.is_some())
            .filter(|model| model.task_kind != TaskKind::Milestone)
            .filter(|model| {
                model
                    .milestone_node_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .is_some()
            })
            .map(|model| model.id)
            .collect();
        if !candidate_task_row_ids.is_empty() {
            let orchestration_models = task_orchestration_state::Entity::find()
                .filter(task_orchestration_state::Column::TaskId.is_in(candidate_task_row_ids))
                .all(db)
                .await?;
            orchestration_state_by_task_row_id = orchestration_models
                .into_iter()
                .map(|model| (model.task_id, model))
                .collect();
        }

        let mut tasks = Vec::with_capacity(models.len());
        for model in models {
            let project_uuid = project_uuid_by_row_id
                .get(&model.project_id)
                .copied()
                .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

            let parent_workspace_id = match model.parent_workspace_id {
                Some(id) => workspace_uuid_by_row_id
                    .get(&id)
                    .copied()
                    .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))
                    .map(Some)?,
                None => None,
            };

            let origin_task_id = match model.origin_task_id {
                Some(id) => origin_task_uuid_by_row_id
                    .get(&id)
                    .copied()
                    .ok_or(DbErr::RecordNotFound("Origin task not found".to_string()))
                    .map(Some)?,
                None => None,
            };

            let shared_task_id = match model.shared_task_id {
                Some(id) => shared_task_uuid_by_row_id
                    .get(&id)
                    .copied()
                    .ok_or(DbErr::RecordNotFound("Shared task not found".to_string()))
                    .map(Some)?,
                None => None,
            };

            let archived_kanban_id = match model.archived_kanban_id {
                Some(id) => archived_kanban_uuid_by_row_id
                    .get(&id)
                    .copied()
                    .ok_or(DbErr::RecordNotFound(
                        "Archived kanban not found".to_string(),
                    ))
                    .map(Some)?,
                None => None,
            };

            let milestone_id = match model.milestone_id {
                Some(id) => milestone_uuid_by_row_id
                    .get(&id)
                    .copied()
                    .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))
                    .map(Some)?,
                None => None,
            };

            let task = Task {
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
            };

            let (has_in_progress_attempt, last_attempt_failed, executor) =
                attempt_status_by_task_row_id
                    .get(&model.id)
                    .cloned()
                    .unwrap_or((false, false, String::new()));

            let dispatch_state = dispatch_state_by_task_row_id.get(&model.id).cloned();

            let orchestration = match model.milestone_id {
                Some(milestone_row_id)
                    if task.task_kind != TaskKind::Milestone
                        && task
                            .milestone_node_id
                            .as_deref()
                            .map(str::trim)
                            .filter(|id| !id.is_empty())
                            .is_some()
                        && milestone_automation_mode_by_row_id
                            .get(&milestone_row_id)
                            .is_some_and(|mode| *mode == MilestoneAutomationMode::Auto) =>
                {
                    let default_turns = project_default_continuation_turns_by_row_id
                        .get(&model.project_id)
                        .copied()
                        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

                    let effective_budget =
                        std::cmp::max(task.continuation_turns_override.unwrap_or(default_turns), 0);
                    let budget_source = if task.continuation_turns_override.is_some() {
                        TaskContinuationBudgetSource::TaskOverride
                    } else {
                        TaskContinuationBudgetSource::ProjectDefault
                    };

                    let state = orchestration_state_by_task_row_id.get(&model.id);
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

                    Some(TaskOrchestrationDiagnostics {
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
                    })
                }
                _ => None,
            };

            tasks.push(TaskWithAttemptStatus {
                task,
                has_in_progress_attempt,
                last_attempt_failed,
                executor,
                dispatch_state,
                orchestration,
            });
        }

        Ok(tasks)
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
        let project_row_id = match ids::project_id_by_uuid(db, project_id).await? {
            Some(row_id) => row_id,
            None => return Ok(Vec::new()),
        };

        let models = task::Entity::find()
            .filter(task::Column::ProjectId.eq(project_row_id))
            .filter(task::Column::ArchivedKanbanId.is_null())
            .order_by_desc(task::Column::CreatedAt)
            .all(db)
            .await?;

        Self::with_attempt_status_bulk(db, models).await
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

        Self::with_attempt_status_bulk(db, models).await
    }

    pub async fn find_filtered_with_attempt_status<C: ConnectionTrait>(
        db: &C,
        project_id: Option<Uuid>,
        include_archived: bool,
        archived_kanban_id: Option<Uuid>,
    ) -> Result<Vec<TaskWithAttemptStatus>, DbErr> {
        let project_row_id = match project_id {
            Some(project_id) => match ids::project_id_by_uuid(db, project_id).await? {
                Some(row_id) => Some(row_id),
                None => return Ok(Vec::new()),
            },
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

        Self::with_attempt_status_bulk(db, models).await
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

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, Database, EntityTrait, Set};
    use sea_orm_migration::MigratorTrait;
    use uuid::Uuid;

    use super::{CreateTask, Task};

    use crate::{
        entities::{archived_kanban, shared_task, task},
        models::{
            execution_process::{CreateExecutionProcess, ExecutionProcess},
            milestone::{
                CreateMilestone, Milestone, MilestoneGraph, MilestoneNode,
                MilestoneNodeBaseStrategy, MilestoneNodeKind, MilestoneNodeLayout,
            },
            project::{CreateProject, Project, UpdateProject},
            session::{CreateSession, Session},
            task_dispatch_state::{TaskDispatchState, UpsertTaskDispatchState},
            task_orchestration_state::TaskOrchestrationState,
            workspace::{CreateWorkspace, Workspace},
        },
        types::{
            ExecutionProcessRunReason, ExecutionProcessStatus, MilestoneAutomationMode,
            TaskDispatchController, TaskDispatchStatus, TaskStatus,
        },
    };

    use executors_protocol::actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    };

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    fn script_executor_action() -> ExecutorAction {
        ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "echo test".to_string(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: None,
            }),
            None,
        )
    }

    #[tokio::test]
    async fn list_hydration_includes_attempt_status_and_dispatch_state() {
        let db = setup_db().await;

        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_running_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Running".to_string(), None),
            task_running_id,
        )
        .await
        .unwrap();

        let task_dispatch_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Dispatch".to_string(), None),
            task_dispatch_id,
        )
        .await
        .unwrap();

        let workspace_id = Uuid::new_v4();
        Workspace::create(
            &db,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            workspace_id,
            task_running_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            &db,
            &CreateSession {
                executor: Some("executor-1".to_string()),
            },
            session_id,
            workspace_id,
        )
        .await
        .unwrap();

        let process_id = Uuid::new_v4();
        ExecutionProcess::create(
            &db,
            &CreateExecutionProcess {
                session_id,
                executor_action: script_executor_action(),
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            process_id,
            &[],
        )
        .await
        .unwrap();

        TaskDispatchState::upsert(
            &db,
            task_dispatch_id,
            &UpsertTaskDispatchState {
                controller: TaskDispatchController::Manual,
                status: TaskDispatchStatus::Idle,
                retry_count: 0,
                max_retries: 3,
                last_error: None,
                blocked_reason: None,
                next_retry_at: None,
                claim_expires_at: None,
            },
        )
        .await
        .unwrap();

        let tasks = Task::find_all_with_attempt_status(&db).await.unwrap();
        let mut by_id = std::collections::HashMap::new();
        for task in tasks {
            by_id.insert(task.id, task);
        }

        let running = by_id.get(&task_running_id).expect("running task");
        assert!(running.has_in_progress_attempt);
        assert!(!running.last_attempt_failed);
        assert_eq!(running.executor, "executor-1");
        assert!(running.dispatch_state.is_none());

        let dispatched = by_id.get(&task_dispatch_id).expect("dispatch task");
        assert!(!dispatched.has_in_progress_attempt);
        assert!(!dispatched.last_attempt_failed);
        assert_eq!(dispatched.executor, "");
        let state = dispatched
            .dispatch_state
            .as_ref()
            .expect("dispatch state present");
        assert_eq!(state.controller, TaskDispatchController::Manual);
        assert_eq!(state.status, TaskDispatchStatus::Idle);

        ExecutionProcess::update_completion(
            &db,
            process_id,
            ExecutionProcessStatus::Failed,
            Some(1),
        )
        .await
        .unwrap();

        let tasks = Task::find_all_with_attempt_status(&db).await.unwrap();
        let failed = tasks
            .into_iter()
            .find(|task| task.id == task_running_id)
            .expect("running task after update");
        assert!(!failed.has_in_progress_attempt);
        assert!(failed.last_attempt_failed);
    }

    #[tokio::test]
    async fn list_hydration_resolves_optional_foreign_keys_and_archived_filter() {
        let db = setup_db().await;

        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let origin_task_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Origin".to_string(), None),
            origin_task_id,
        )
        .await
        .unwrap();

        let workspace_id = Uuid::new_v4();
        Workspace::create(
            &db,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            workspace_id,
            origin_task_id,
        )
        .await
        .unwrap();

        let shared_task_uuid = Uuid::new_v4();
        let now = chrono::Utc::now();
        shared_task::ActiveModel {
            uuid: Set(shared_task_uuid),
            remote_project_id: Set(Uuid::new_v4()),
            title: Set("Shared task".to_string()),
            description: Set(None),
            status: Set("todo".to_string()),
            assignee_user_id: Set(None),
            assignee_first_name: Set(None),
            assignee_last_name: Set(None),
            assignee_username: Set(None),
            version: Set(1),
            last_event_seq: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        let follow_up_task_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask {
                project_id,
                title: "Follow up".to_string(),
                description: None,
                status: Some(TaskStatus::Todo),
                task_kind: None,
                milestone_id: None,
                milestone_node_id: None,
                parent_workspace_id: Some(workspace_id),
                origin_task_id: Some(origin_task_id),
                created_by_kind: None,
                image_ids: None,
                shared_task_id: Some(shared_task_uuid),
            },
            follow_up_task_id,
        )
        .await
        .unwrap();

        let project_row_id = crate::models::ids::project_id_by_uuid(&db, project_id)
            .await
            .unwrap()
            .expect("project row id");

        let archived_kanban_uuid = Uuid::new_v4();
        let archived_kanban_model = archived_kanban::ActiveModel {
            uuid: Set(archived_kanban_uuid),
            project_id: Set(project_row_id),
            title: Set("Archived".to_string()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        let follow_up_row_id = crate::models::ids::task_id_by_uuid(&db, follow_up_task_id)
            .await
            .unwrap()
            .expect("task row id");
        let record = task::Entity::find_by_id(follow_up_row_id)
            .one(&db)
            .await
            .unwrap()
            .expect("task record");
        let mut active: task::ActiveModel = record.into();
        active.archived_kanban_id = Set(Some(archived_kanban_model.id));
        active.update(&db).await.unwrap();

        let tasks = Task::find_filtered_with_attempt_status(
            &db,
            Some(project_id),
            true,
            Some(archived_kanban_uuid),
        )
        .await
        .unwrap();
        assert_eq!(tasks.len(), 1);
        let task = &tasks[0];
        assert_eq!(task.id, follow_up_task_id);
        assert_eq!(task.origin_task_id, Some(origin_task_id));
        assert_eq!(task.parent_workspace_id, Some(workspace_id));
        assert_eq!(task.shared_task_id, Some(shared_task_uuid));
        assert_eq!(task.archived_kanban_id, Some(archived_kanban_uuid));
    }

    #[tokio::test]
    async fn list_hydration_computes_orchestration_diagnostics_for_auto_milestones() {
        let db = setup_db().await;

        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        Project::update(
            &db,
            project_id,
            &UpdateProject {
                name: None,
                dev_script: None,
                dev_script_working_dir: None,
                default_agent_working_dir: None,
                git_no_verify_override: None,
                scheduler_max_concurrent: None,
                scheduler_max_retries: None,
                default_continuation_turns: Some(4),
                after_prepare_hook: None,
                before_cleanup_hook: None,
            },
        )
        .await
        .unwrap();

        let node_task_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Node task".to_string(), None),
            node_task_id,
        )
        .await
        .unwrap();

        let graph = MilestoneGraph {
            nodes: vec![MilestoneNode {
                id: "node-1".to_string(),
                task_id: node_task_id,
                kind: MilestoneNodeKind::Task,
                phase: 0,
                executor_profile_id: None,
                base_strategy: MilestoneNodeBaseStrategy::Topology,
                instructions: None,
                requires_approval: None,
                layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                status: None,
            }],
            edges: Vec::new(),
        };

        let milestone_id = Uuid::new_v4();
        Milestone::create(
            &db,
            &CreateMilestone {
                project_id,
                title: "Auto milestone".to_string(),
                description: None,
                objective: None,
                definition_of_done: None,
                default_executor_profile_id: None,
                automation_mode: Some(MilestoneAutomationMode::Auto),
                status: None,
                baseline_ref: Some("main".to_string()),
                schema_version: 1,
                graph,
            },
            milestone_id,
        )
        .await
        .unwrap();

        TaskOrchestrationState::increment_continuation_turns_used(
            &db,
            node_task_id,
            Uuid::new_v4(),
        )
        .await
        .unwrap();

        let tasks = Task::find_by_project_id_with_attempt_status(&db, project_id)
            .await
            .unwrap();
        let node = tasks
            .into_iter()
            .find(|task| task.id == node_task_id)
            .expect("node task");
        let orch = node.orchestration.expect("orchestration");
        assert_eq!(orch.continuation.turn_budget, 4);
        assert_eq!(orch.continuation.turns_used, 1);
        assert_eq!(orch.continuation.turns_remaining, 3);
        assert!(matches!(
            orch.continuation.budget_source,
            super::TaskContinuationBudgetSource::ProjectDefault
        ));
    }
}
