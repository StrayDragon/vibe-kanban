use chrono::{DateTime, Utc};
use executors::{
    actions::{ExecutorAction, ExecutorActionType},
    profile::ExecutorProfileId,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
    sea_query::{Expr, ExprTrait, JoinType, Order, Query},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use super::{
    execution_process_repo_state::{CreateExecutionProcessRepoState, ExecutionProcessRepoState},
    project::Project,
    repo::Repo,
    session::Session,
    task::Task,
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
pub use crate::types::{ExecutionProcessRunReason, ExecutionProcessStatus};
use crate::{
    entities::{
        coding_agent_turn, execution_process, execution_process_repo_state, repo, session, task,
        workspace,
    },
    events::{
        EVENT_EXECUTION_PROCESS_CREATED, EVENT_EXECUTION_PROCESS_UPDATED,
        ExecutionProcessEventPayload,
    },
    models::{event_outbox::EventOutbox, ids},
};

#[derive(Debug, Error)]
pub enum ExecutionProcessError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Execution process not found")]
    ExecutionProcessNotFound,
    #[error("Failed to create execution process: {0}")]
    CreateFailed(String),
    #[error("Failed to update execution process: {0}")]
    UpdateFailed(String),
    #[error("Invalid executor action format")]
    InvalidExecutorAction,
    #[error("Validation error: {0}")]
    ValidationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExecutionProcess {
    pub id: Uuid,
    pub session_id: Uuid,
    pub run_reason: ExecutionProcessRunReason,
    #[ts(type = "ExecutorAction")]
    pub executor_action: ExecutorActionField,
    pub status: ExecutionProcessStatus,
    pub exit_code: Option<i64>,
    /// dropped: true if this process is excluded from the current
    /// history view (due to restore/trimming). Hidden from logs/timeline;
    /// still listed in the Processes tab.
    pub dropped: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateExecutionProcess {
    pub session_id: Uuid,
    pub executor_action: ExecutorAction,
    pub run_reason: ExecutionProcessRunReason,
}

#[derive(Debug, Deserialize, TS)]
#[allow(dead_code)]
pub struct UpdateExecutionProcess {
    pub status: Option<ExecutionProcessStatus>,
    pub exit_code: Option<i64>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct ExecutionContext {
    pub execution_process: ExecutionProcess,
    pub session: Session,
    pub workspace: Workspace,
    pub task: Task,
    pub project: Project,
    pub repos: Vec<Repo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExecutorActionField {
    ExecutorAction(ExecutorAction),
    Other(Value),
}

#[derive(Debug, Clone)]
pub struct MissingBeforeContext {
    pub id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
    pub prev_after_head_commit: Option<String>,
    pub target_branch: String,
    pub repo_path: Option<String>,
}

impl ExecutionProcess {
    async fn from_model<C: ConnectionTrait>(
        db: &C,
        model: execution_process::Model,
    ) -> Result<Self, DbErr> {
        let executor_action = serde_json::from_value(model.executor_action.clone())
            .unwrap_or(ExecutorActionField::Other(model.executor_action));
        let session_id = ids::session_uuid_by_id(db, model.session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        Ok(Self {
            id: model.uuid,
            session_id,
            run_reason: model.run_reason,
            executor_action,
            status: model.status,
            exit_code: model.exit_code,
            dropped: model.dropped,
            started_at: model.started_at.into(),
            completed_at: model.completed_at.map(Into::into),
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        })
    }

    /// Find execution process by ID
    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = execution_process::Entity::find()
            .filter(execution_process::Column::Uuid.eq(id))
            .one(db)
            .await?;
        if let Some(model) = record {
            return Ok(Some(Self::from_model(db, model).await?));
        }
        Ok(None)
    }

    /// Context for backfilling before_head_commit for legacy rows
    /// List processes that have after_head_commit set but missing before_head_commit, with join context
    pub async fn list_missing_before_context<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<MissingBeforeContext>, DbErr> {
        let states = execution_process_repo_state::Entity::find()
            .filter(execution_process_repo_state::Column::BeforeHeadCommit.is_null())
            .filter(execution_process_repo_state::Column::AfterHeadCommit.is_not_null())
            .all(db)
            .await?;

        let mut result = Vec::new();
        for state in states {
            let process = match execution_process::Entity::find_by_id(state.execution_process_id)
                .one(db)
                .await?
            {
                Some(proc) => proc,
                None => continue,
            };
            let session = match session::Entity::find_by_id(process.session_id)
                .one(db)
                .await?
            {
                Some(session) => session,
                None => continue,
            };
            let workspace = match workspace::Entity::find_by_id(session.workspace_id)
                .one(db)
                .await?
            {
                Some(workspace) => workspace,
                None => continue,
            };

            let prev_query = Query::select()
                .column(execution_process_repo_state::Column::AfterHeadCommit)
                .from(execution_process_repo_state::Entity)
                .join(
                    JoinType::InnerJoin,
                    execution_process::Entity,
                    Expr::col((execution_process::Entity, execution_process::Column::Id)).equals((
                        execution_process_repo_state::Entity,
                        execution_process_repo_state::Column::ExecutionProcessId,
                    )),
                )
                .and_where(
                    Expr::col((
                        execution_process::Entity,
                        execution_process::Column::SessionId,
                    ))
                    .eq(process.session_id),
                )
                .and_where(
                    Expr::col((
                        execution_process_repo_state::Entity,
                        execution_process_repo_state::Column::RepoId,
                    ))
                    .eq(state.repo_id),
                )
                .and_where(
                    Expr::col((
                        execution_process::Entity,
                        execution_process::Column::CreatedAt,
                    ))
                    .lt(process.created_at),
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

            let prev_after_head_commit = db
                .query_one(&prev_query)
                .await?
                .and_then(|row| row.try_get("", "after_head_commit").ok());

            let workspace_repo_row = crate::entities::workspace_repo::Entity::find()
                .filter(crate::entities::workspace_repo::Column::WorkspaceId.eq(workspace.id))
                .filter(crate::entities::workspace_repo::Column::RepoId.eq(state.repo_id))
                .one(db)
                .await?;

            let target_branch = workspace_repo_row
                .map(|row| row.target_branch)
                .unwrap_or_else(|| "".to_string());

            let repo_path = repo::Entity::find_by_id(state.repo_id)
                .one(db)
                .await?
                .map(|row| row.path);

            let session_uuid = ids::session_uuid_by_id(db, process.session_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;
            let workspace_uuid = ids::workspace_uuid_by_id(db, session.workspace_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
            let repo_uuid = ids::repo_uuid_by_id(db, state.repo_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

            result.push(MissingBeforeContext {
                id: process.uuid,
                session_id: session_uuid,
                workspace_id: workspace_uuid,
                repo_id: repo_uuid,
                prev_after_head_commit,
                target_branch,
                repo_path,
            });
        }

        Ok(result)
    }

    /// Find all execution processes for a session (optionally include soft-deleted)
    pub async fn find_by_session_id<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
        show_soft_deleted: bool,
    ) -> Result<Vec<Self>, DbErr> {
        let session_row_id = ids::session_id_by_uuid(db, session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let mut query = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.eq(session_row_id));
        if !show_soft_deleted {
            query = query.filter(execution_process::Column::Dropped.eq(false));
        }

        let records = query
            .order_by_asc(execution_process::Column::CreatedAt)
            .all(db)
            .await?;

        let mut processes = Vec::with_capacity(records.len());
        for model in records {
            processes.push(Self::from_model(db, model).await?);
        }
        Ok(processes)
    }

    /// Find running execution processes
    pub async fn find_running<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let records = execution_process::Entity::find()
            .filter(execution_process::Column::Status.eq(ExecutionProcessStatus::Running))
            .order_by_asc(execution_process::Column::CreatedAt)
            .all(db)
            .await?;

        let mut processes = Vec::with_capacity(records.len());
        for model in records {
            processes.push(Self::from_model(db, model).await?);
        }
        Ok(processes)
    }

    /// Find running dev servers for a specific project
    pub async fn find_running_dev_servers_by_project<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let task_ids: Vec<i64> = task::Entity::find()
            .select_only()
            .column(task::Column::Id)
            .filter(task::Column::ProjectId.eq(project_row_id))
            .into_tuple()
            .all(db)
            .await?;

        if task_ids.is_empty() {
            return Ok(Vec::new());
        }

        let workspace_ids: Vec<i64> = workspace::Entity::find()
            .select_only()
            .column(workspace::Column::Id)
            .filter(workspace::Column::TaskId.is_in(task_ids))
            .into_tuple()
            .all(db)
            .await?;

        if workspace_ids.is_empty() {
            return Ok(Vec::new());
        }

        let session_ids: Vec<i64> = session::Entity::find()
            .select_only()
            .column(session::Column::Id)
            .filter(session::Column::WorkspaceId.is_in(workspace_ids))
            .into_tuple()
            .all(db)
            .await?;

        if session_ids.is_empty() {
            return Ok(Vec::new());
        }

        let records = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.is_in(session_ids))
            .filter(execution_process::Column::Status.eq(ExecutionProcessStatus::Running))
            .filter(execution_process::Column::RunReason.eq(ExecutionProcessRunReason::DevServer))
            .order_by_asc(execution_process::Column::CreatedAt)
            .all(db)
            .await?;

        let mut processes = Vec::with_capacity(records.len());
        for model in records {
            processes.push(Self::from_model(db, model).await?);
        }
        Ok(processes)
    }

    /// Check if there are running processes (excluding dev servers) for a workspace (across all sessions)
    pub async fn has_running_non_dev_server_processes_for_workspace<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<bool, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let session_ids: Vec<i64> = session::Entity::find()
            .select_only()
            .column(session::Column::Id)
            .filter(session::Column::WorkspaceId.eq(workspace_row_id))
            .into_tuple()
            .all(db)
            .await?;

        if session_ids.is_empty() {
            return Ok(false);
        }

        let exists = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.is_in(session_ids))
            .filter(execution_process::Column::Status.eq(ExecutionProcessStatus::Running))
            .filter(execution_process::Column::RunReason.ne(ExecutionProcessRunReason::DevServer))
            .one(db)
            .await?
            .is_some();

        Ok(exists)
    }

    /// Find running dev servers for a specific workspace (across all sessions)
    pub async fn find_running_dev_servers_by_workspace<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let session_ids: Vec<i64> = session::Entity::find()
            .select_only()
            .column(session::Column::Id)
            .filter(session::Column::WorkspaceId.eq(workspace_row_id))
            .into_tuple()
            .all(db)
            .await?;

        if session_ids.is_empty() {
            return Ok(Vec::new());
        }

        let records = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.is_in(session_ids))
            .filter(execution_process::Column::Status.eq(ExecutionProcessStatus::Running))
            .filter(execution_process::Column::RunReason.eq(ExecutionProcessRunReason::DevServer))
            .order_by_desc(execution_process::Column::CreatedAt)
            .all(db)
            .await?;

        let mut processes = Vec::with_capacity(records.len());
        for model in records {
            processes.push(Self::from_model(db, model).await?);
        }
        Ok(processes)
    }

    /// Find latest coding_agent_turn agent_session_id by session (simple scalar query)
    pub async fn find_latest_coding_agent_turn_session_id<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
    ) -> Result<Option<String>, DbErr> {
        let session_row_id = ids::session_id_by_uuid(db, session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let latest_process = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.eq(session_row_id))
            .filter(execution_process::Column::RunReason.eq(ExecutionProcessRunReason::CodingAgent))
            .filter(execution_process::Column::Dropped.eq(false))
            .order_by_desc(execution_process::Column::CreatedAt)
            .one(db)
            .await?;

        let Some(process) = latest_process else {
            return Ok(None);
        };

        let turn = coding_agent_turn::Entity::find()
            .filter(coding_agent_turn::Column::ExecutionProcessId.eq(process.id))
            .filter(coding_agent_turn::Column::AgentSessionId.is_not_null())
            .order_by_desc(coding_agent_turn::Column::CreatedAt)
            .one(db)
            .await?;

        Ok(turn.and_then(|row| row.agent_session_id))
    }

    /// Find latest execution process by session and run reason
    pub async fn find_latest_by_session_and_run_reason<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
        run_reason: &ExecutionProcessRunReason,
    ) -> Result<Option<Self>, DbErr> {
        let session_row_id = ids::session_id_by_uuid(db, session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let record = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.eq(session_row_id))
            .filter(execution_process::Column::RunReason.eq(run_reason.clone()))
            .filter(execution_process::Column::Dropped.eq(false))
            .order_by_desc(execution_process::Column::CreatedAt)
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    /// Find latest execution process by workspace and run reason (across all sessions)
    pub async fn find_latest_by_workspace_and_run_reason<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        run_reason: &ExecutionProcessRunReason,
    ) -> Result<Option<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let session_ids: Vec<i64> = session::Entity::find()
            .select_only()
            .column(session::Column::Id)
            .filter(session::Column::WorkspaceId.eq(workspace_row_id))
            .into_tuple()
            .all(db)
            .await?;

        if session_ids.is_empty() {
            return Ok(None);
        }

        let record = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.is_in(session_ids))
            .filter(execution_process::Column::RunReason.eq(run_reason.clone()))
            .filter(execution_process::Column::Dropped.eq(false))
            .order_by_desc(execution_process::Column::CreatedAt)
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    /// Create a new execution process
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateExecutionProcess,
        process_id: Uuid,
        repo_states: &[CreateExecutionProcessRepoState],
    ) -> Result<Self, DbErr> {
        let session_row_id = ids::session_id_by_uuid(db, data.session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;
        let now = Utc::now();
        let executor_action_value = serde_json::to_value(&data.executor_action)
            .map_err(|err| DbErr::Custom(err.to_string()))?;

        let active = execution_process::ActiveModel {
            uuid: Set(process_id),
            session_id: Set(session_row_id),
            run_reason: Set(data.run_reason.clone()),
            executor_action: Set(executor_action_value),
            status: Set(ExecutionProcessStatus::Running),
            exit_code: Set(None),
            dropped: Set(false),
            started_at: Set(now.into()),
            completed_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        active.insert(db).await?;
        ExecutionProcessRepoState::create_many(db, process_id, repo_states).await?;
        let payload = serde_json::to_value(ExecutionProcessEventPayload {
            process_id,
            session_id: data.session_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(
            db,
            EVENT_EXECUTION_PROCESS_CREATED,
            "execution_process",
            process_id,
            payload,
        )
        .await?;

        Self::find_by_id(db, process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))
    }

    pub async fn was_stopped<C: ConnectionTrait>(db: &C, id: Uuid) -> bool {
        if let Ok(Some(exp_process)) = Self::find_by_id(db, id).await {
            return matches!(
                exp_process.status,
                ExecutionProcessStatus::Killed | ExecutionProcessStatus::Completed
            );
        }
        false
    }

    /// Update execution process status and completion info
    pub async fn update_completion<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        status: ExecutionProcessStatus,
        exit_code: Option<i64>,
    ) -> Result<(), DbErr> {
        let completed_at = if matches!(status, ExecutionProcessStatus::Running) {
            None
        } else {
            Some(Utc::now())
        };

        let record = execution_process::Entity::find()
            .filter(execution_process::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let session_uuid = ids::session_uuid_by_id(db, record.session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;
        let mut active: execution_process::ActiveModel = record.into();
        active.status = Set(status);
        active.exit_code = Set(exit_code);
        active.completed_at = Set(completed_at.map(Into::into));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let payload = serde_json::to_value(ExecutionProcessEventPayload {
            process_id: id,
            session_id: session_uuid,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(
            db,
            EVENT_EXECUTION_PROCESS_UPDATED,
            "execution_process",
            id,
            payload,
        )
        .await?;
        Ok(())
    }

    pub fn executor_action(&self) -> Result<&ExecutorAction, anyhow::Error> {
        match &self.executor_action {
            ExecutorActionField::ExecutorAction(action) => Ok(action),
            ExecutorActionField::Other(_) => Err(anyhow::anyhow!(
                "Executor action is not a valid ExecutorAction JSON object"
            )),
        }
    }

    /// Soft-drop processes at and after the specified boundary (inclusive)
    pub async fn drop_at_and_after<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
        boundary_process_id: Uuid,
    ) -> Result<i64, DbErr> {
        let session_row_id = ids::session_id_by_uuid(db, session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;
        let boundary = execution_process::Entity::find()
            .filter(execution_process::Column::Uuid.eq(boundary_process_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let affected = execution_process::Entity::find()
            .filter(execution_process::Column::SessionId.eq(session_row_id))
            .filter(execution_process::Column::CreatedAt.gte(boundary.created_at))
            .filter(execution_process::Column::Dropped.eq(false))
            .all(db)
            .await?;

        let result = execution_process::Entity::update_many()
            .col_expr(execution_process::Column::Dropped, Expr::value(true))
            .filter(execution_process::Column::SessionId.eq(session_row_id))
            .filter(execution_process::Column::CreatedAt.gte(boundary.created_at))
            .filter(execution_process::Column::Dropped.eq(false))
            .exec(db)
            .await?;

        for process in affected {
            let payload = serde_json::to_value(ExecutionProcessEventPayload {
                process_id: process.uuid,
                session_id,
            })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(
                db,
                EVENT_EXECUTION_PROCESS_UPDATED,
                "execution_process",
                process.uuid,
                payload,
            )
            .await?;
        }
        Ok(result.rows_affected as i64)
    }

    /// Find the previous process's after_head_commit before the given boundary process
    /// for a specific repository
    pub async fn find_prev_after_head_commit<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
        boundary_process_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Option<String>, DbErr> {
        let session_row_id = ids::session_id_by_uuid(db, session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;
        let boundary = execution_process::Entity::find()
            .filter(execution_process::Column::Uuid.eq(boundary_process_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let query = Query::select()
            .column(execution_process_repo_state::Column::AfterHeadCommit)
            .from(execution_process_repo_state::Entity)
            .join(
                JoinType::InnerJoin,
                execution_process::Entity,
                Expr::col((execution_process::Entity, execution_process::Column::Id)).equals((
                    execution_process_repo_state::Entity,
                    execution_process_repo_state::Column::ExecutionProcessId,
                )),
            )
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::SessionId,
                ))
                .eq(session_row_id),
            )
            .and_where(
                Expr::col((
                    execution_process_repo_state::Entity,
                    execution_process_repo_state::Column::RepoId,
                ))
                .eq(repo_row_id),
            )
            .and_where(
                Expr::col((
                    execution_process::Entity,
                    execution_process::Column::CreatedAt,
                ))
                .lt(boundary.created_at),
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

        let result = db
            .query_one(&query)
            .await?
            .and_then(|row| row.try_get("", "after_head_commit").ok());

        Ok(result)
    }

    /// Get the parent Session for this execution process
    pub async fn parent_session<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> Result<Option<Session>, DbErr> {
        Session::find_by_id(db, self.session_id).await
    }

    /// Get both the parent Workspace and Session for this execution process
    pub async fn parent_workspace_and_session<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> Result<Option<(Workspace, Session)>, DbErr> {
        let session = match Session::find_by_id(db, self.session_id).await? {
            Some(s) => s,
            None => return Ok(None),
        };
        let workspace = match Workspace::find_by_id(db, session.workspace_id).await? {
            Some(w) => w,
            None => return Ok(None),
        };
        Ok(Some((workspace, session)))
    }

    /// Load execution context with related session, workspace, task, project, and repos
    pub async fn load_context<C: ConnectionTrait>(
        db: &C,
        exec_id: Uuid,
    ) -> Result<ExecutionContext, DbErr> {
        let execution_process =
            Self::find_by_id(db, exec_id)
                .await?
                .ok_or(DbErr::RecordNotFound(
                    "Execution process not found".to_string(),
                ))?;

        let session = Session::find_by_id(db, execution_process.session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let workspace = Workspace::find_by_id(db, session.workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let task = Task::find_by_id(db, workspace.task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let project = Project::find_by_id(db, task.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let repos = WorkspaceRepo::find_repos_for_workspace(db, workspace.id).await?;

        Ok(ExecutionContext {
            execution_process,
            session,
            workspace,
            task,
            project,
            repos,
        })
    }

    /// Fetch the latest CodingAgent executor profile for a session
    pub async fn latest_executor_profile_for_session<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
    ) -> Result<ExecutorProfileId, ExecutionProcessError> {
        let latest_execution_process = Self::find_latest_by_session_and_run_reason(
            db,
            session_id,
            &ExecutionProcessRunReason::CodingAgent,
        )
        .await?
        .ok_or_else(|| {
            ExecutionProcessError::ValidationError(
                "Couldn't find initial coding agent process, has it run yet?".to_string(),
            )
        })?;

        let action = latest_execution_process
            .executor_action()
            .map_err(|e| ExecutionProcessError::ValidationError(e.to_string()))?;

        match &action.typ {
            ExecutorActionType::CodingAgentInitialRequest(request) => {
                Ok(request.executor_profile_id.clone())
            }
            ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                Ok(request.executor_profile_id.clone())
            }
            _ => Err(ExecutionProcessError::ValidationError(
                "Couldn't find profile from initial request".to_string(),
            )),
        }
    }
}
