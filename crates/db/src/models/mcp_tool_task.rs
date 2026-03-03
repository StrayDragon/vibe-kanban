use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

use crate::entities::mcp_tool_task;

#[derive(Debug, Clone)]
pub struct McpToolTask {
    pub db_id: i64,
    pub task_id: String,
    pub created_by_client_id: Option<String>,
    pub tool_name: String,
    pub tool_arguments_json: serde_json::Value,
    pub status: String,
    pub status_message: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub kanban_task_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub resumable: bool,
    pub ttl_ms: Option<i64>,
    pub poll_interval_ms: Option<i64>,
    pub result_json: Option<serde_json::Value>,
    pub error_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl McpToolTask {
    fn from_model(model: mcp_tool_task::Model) -> Self {
        Self {
            db_id: model.id,
            task_id: model.task_id,
            created_by_client_id: model.created_by_client_id,
            tool_name: model.tool_name,
            tool_arguments_json: model.tool_arguments_json,
            status: model.status,
            status_message: model.status_message,
            attempt_id: model.attempt_id,
            kanban_task_id: model.kanban_task_id,
            project_id: model.project_id,
            resumable: model.resumable,
            ttl_ms: model.ttl_ms,
            poll_interval_ms: model.poll_interval_ms,
            result_json: model.result_json,
            error_json: model.error_json,
            created_at: model.created_at.into(),
            last_updated_at: model.last_updated_at.into(),
            expires_at: model.expires_at.map(Into::into),
        }
    }
}

fn compute_expires_at(
    last_updated_at: DateTime<Utc>,
    ttl_ms: Option<i64>,
) -> Option<DateTime<Utc>> {
    let ttl_ms = ttl_ms?;
    if ttl_ms <= 0 {
        return None;
    }
    Some(last_updated_at + Duration::milliseconds(ttl_ms))
}

pub async fn insert_working<C: ConnectionTrait>(
    db: &C,
    task_id: String,
    created_by_client_id: Option<String>,
    tool_name: String,
    tool_arguments_json: serde_json::Value,
    attempt_id: Option<Uuid>,
    kanban_task_id: Option<Uuid>,
    project_id: Option<Uuid>,
    ttl_ms: Option<i64>,
    poll_interval_ms: Option<i64>,
    resumable: bool,
) -> Result<McpToolTask, DbErr> {
    let now = Utc::now();
    let active = mcp_tool_task::ActiveModel {
        task_id: Set(task_id),
        created_by_client_id: Set(created_by_client_id),
        tool_name: Set(tool_name),
        tool_arguments_json: Set(tool_arguments_json),
        status: Set("working".to_string()),
        status_message: Set(None),
        attempt_id: Set(attempt_id),
        kanban_task_id: Set(kanban_task_id),
        project_id: Set(project_id),
        resumable: Set(resumable),
        ttl_ms: Set(ttl_ms),
        poll_interval_ms: Set(poll_interval_ms),
        result_json: Set(None),
        error_json: Set(None),
        created_at: Set(now.into()),
        last_updated_at: Set(now.into()),
        expires_at: Set(None),
        ..Default::default()
    };

    let inserted = active.insert(db).await?;
    Ok(McpToolTask::from_model(inserted))
}

pub async fn find_by_task_id<C: ConnectionTrait>(
    db: &C,
    task_id: &str,
) -> Result<Option<McpToolTask>, DbErr> {
    let record = mcp_tool_task::Entity::find()
        .filter(mcp_tool_task::Column::TaskId.eq(task_id))
        .one(db)
        .await?;
    Ok(record.map(McpToolTask::from_model))
}

pub async fn list<C: ConnectionTrait>(
    db: &C,
    status: Option<&str>,
    attempt_id: Option<Uuid>,
    kanban_task_id: Option<Uuid>,
    project_id: Option<Uuid>,
    limit: u64,
    cursor: Option<i64>,
) -> Result<(Vec<McpToolTask>, Option<i64>), DbErr> {
    let limit = limit.clamp(1, 200);
    let mut query = mcp_tool_task::Entity::find();
    if let Some(status) = status {
        query = query.filter(mcp_tool_task::Column::Status.eq(status));
    }
    if let Some(attempt_id) = attempt_id {
        query = query.filter(mcp_tool_task::Column::AttemptId.eq(attempt_id));
    }
    if let Some(kanban_task_id) = kanban_task_id {
        query = query.filter(mcp_tool_task::Column::KanbanTaskId.eq(kanban_task_id));
    }
    if let Some(project_id) = project_id {
        query = query.filter(mcp_tool_task::Column::ProjectId.eq(project_id));
    }
    if let Some(cursor) = cursor {
        query = query.filter(mcp_tool_task::Column::Id.lt(cursor));
    }

    let mut records = query
        .order_by_desc(mcp_tool_task::Column::Id)
        .limit(limit)
        .all(db)
        .await?;

    let next_cursor = records.last().map(|r| r.id);
    let tasks = records
        .drain(..)
        .map(McpToolTask::from_model)
        .collect::<Vec<_>>();
    Ok((tasks, next_cursor))
}

pub async fn update_status<C: ConnectionTrait>(
    db: &C,
    task_id: &str,
    status: &str,
    status_message: Option<String>,
) -> Result<McpToolTask, DbErr> {
    let record = mcp_tool_task::Entity::find()
        .filter(mcp_tool_task::Column::TaskId.eq(task_id))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

    let now = Utc::now();
    let expires_at = if matches!(status, "completed" | "failed" | "cancelled") {
        compute_expires_at(now, record.ttl_ms).map(Into::into)
    } else {
        record.expires_at
    };

    let mut active: mcp_tool_task::ActiveModel = record.into();
    active.status = Set(status.to_string());
    active.status_message = Set(status_message);
    active.last_updated_at = Set(now.into());
    active.expires_at = Set(expires_at);

    let updated = active.update(db).await?;
    Ok(McpToolTask::from_model(updated))
}

pub async fn complete_with_result<C: ConnectionTrait>(
    db: &C,
    task_id: &str,
    result_json: serde_json::Value,
    status_message: Option<String>,
) -> Result<McpToolTask, DbErr> {
    finish_with_payload(db, task_id, "completed", result_json, status_message).await
}

pub async fn finish_with_payload<C: ConnectionTrait>(
    db: &C,
    task_id: &str,
    status: &str,
    payload_json: serde_json::Value,
    status_message: Option<String>,
) -> Result<McpToolTask, DbErr> {
    let record = mcp_tool_task::Entity::find()
        .filter(mcp_tool_task::Column::TaskId.eq(task_id))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

    let now = Utc::now();
    let expires_at = compute_expires_at(now, record.ttl_ms).map(Into::into);

    let mut active: mcp_tool_task::ActiveModel = record.into();
    active.status = Set(status.to_string());
    active.status_message = Set(status_message);
    active.result_json = Set(Some(payload_json));
    active.error_json = Set(None);
    active.last_updated_at = Set(now.into());
    active.expires_at = Set(expires_at);

    let updated = active.update(db).await?;
    Ok(McpToolTask::from_model(updated))
}

pub async fn fail_with_error<C: ConnectionTrait>(
    db: &C,
    task_id: &str,
    error_json: serde_json::Value,
    status_message: Option<String>,
) -> Result<McpToolTask, DbErr> {
    let record = mcp_tool_task::Entity::find()
        .filter(mcp_tool_task::Column::TaskId.eq(task_id))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

    let now = Utc::now();
    let expires_at = compute_expires_at(now, record.ttl_ms).map(Into::into);

    let mut active: mcp_tool_task::ActiveModel = record.into();
    active.status = Set("failed".to_string());
    active.status_message = Set(status_message);
    active.result_json = Set(None);
    active.error_json = Set(Some(error_json));
    active.last_updated_at = Set(now.into());
    active.expires_at = Set(expires_at);

    let updated = active.update(db).await?;
    Ok(McpToolTask::from_model(updated))
}

pub async fn delete_expired<C: ConnectionTrait>(db: &C) -> Result<u64, DbErr> {
    let now = Utc::now();
    let result = mcp_tool_task::Entity::delete_many()
        .filter(mcp_tool_task::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

pub async fn list_working_resumable<C: ConnectionTrait>(db: &C) -> Result<Vec<McpToolTask>, DbErr> {
    let records = mcp_tool_task::Entity::find()
        .filter(mcp_tool_task::Column::Status.eq("working"))
        .filter(mcp_tool_task::Column::Resumable.eq(true))
        .order_by_desc(mcp_tool_task::Column::Id)
        .all(db)
        .await?;
    Ok(records.into_iter().map(McpToolTask::from_model).collect())
}

pub async fn list_working<C: ConnectionTrait>(db: &C) -> Result<Vec<McpToolTask>, DbErr> {
    let records = mcp_tool_task::Entity::find()
        .filter(mcp_tool_task::Column::Status.eq("working"))
        .order_by_desc(mcp_tool_task::Column::Id)
        .all(db)
        .await?;
    Ok(records.into_iter().map(McpToolTask::from_model).collect())
}
