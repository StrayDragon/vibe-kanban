use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use utils::approvals::ApprovalStatus;
use uuid::Uuid;

use crate::entities::approval;

#[derive(Debug, Clone)]
pub struct Approval {
    pub db_id: i64,
    pub id: String,
    pub attempt_id: Uuid,
    pub execution_process_id: Uuid,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_call_id: String,
    pub status: ApprovalStatus,
    pub created_at: DateTime<Utc>,
    pub timeout_at: DateTime<Utc>,
    pub responded_at: Option<DateTime<Utc>>,
    pub responded_by_client_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

fn status_from_db(raw: &str, denied_reason: Option<String>) -> Result<ApprovalStatus, DbErr> {
    match raw {
        "pending" => Ok(ApprovalStatus::Pending),
        "approved" => Ok(ApprovalStatus::Approved),
        "denied" => Ok(ApprovalStatus::Denied {
            reason: denied_reason,
        }),
        "timed_out" => Ok(ApprovalStatus::TimedOut),
        other => Err(DbErr::Custom(format!("unknown approval status: {other}"))),
    }
}

fn status_to_db(status: &ApprovalStatus) -> (&'static str, Option<String>) {
    match status {
        ApprovalStatus::Pending => ("pending", None),
        ApprovalStatus::Approved => ("approved", None),
        ApprovalStatus::Denied { reason } => ("denied", reason.clone()),
        ApprovalStatus::TimedOut => ("timed_out", None),
    }
}

impl Approval {
    fn from_model(model: approval::Model) -> Result<Self, DbErr> {
        Ok(Self {
            db_id: model.id,
            id: model.uuid.to_string(),
            attempt_id: model.attempt_id,
            execution_process_id: model.execution_process_id,
            tool_name: model.tool_name,
            tool_input: model.tool_input_json,
            tool_call_id: model.tool_call_id,
            status: status_from_db(&model.status, model.denied_reason)?,
            created_at: model.created_at.into(),
            timeout_at: model.timeout_at.into(),
            responded_at: model.responded_at.map(Into::into),
            responded_by_client_id: model.responded_by_client_id,
            updated_at: model.updated_at.into(),
        })
    }
}

pub async fn get_by_id<C: ConnectionTrait>(
    db: &C,
    approval_id: Uuid,
) -> Result<Option<Approval>, DbErr> {
    let record = approval::Entity::find()
        .filter(approval::Column::Uuid.eq(approval_id))
        .one(db)
        .await?;
    record.map(Approval::from_model).transpose()
}

pub async fn find_pending_by_execution_tool_call<C: ConnectionTrait>(
    db: &C,
    execution_process_id: Uuid,
    tool_call_id: &str,
) -> Result<Option<Approval>, DbErr> {
    let record = approval::Entity::find()
        .filter(approval::Column::ExecutionProcessId.eq(execution_process_id))
        .filter(approval::Column::ToolCallId.eq(tool_call_id))
        .filter(approval::Column::Status.eq("pending"))
        .one(db)
        .await?;
    record.map(Approval::from_model).transpose()
}

pub async fn insert_pending<C: ConnectionTrait>(
    db: &C,
    approval_id: Uuid,
    attempt_id: Uuid,
    execution_process_id: Uuid,
    tool_name: String,
    tool_input: serde_json::Value,
    tool_call_id: String,
    created_at: DateTime<Utc>,
    timeout_at: DateTime<Utc>,
) -> Result<Approval, DbErr> {
    let now = Utc::now();
    let active = approval::ActiveModel {
        uuid: Set(approval_id),
        attempt_id: Set(attempt_id),
        execution_process_id: Set(execution_process_id),
        tool_call_id: Set(tool_call_id),
        tool_name: Set(tool_name),
        tool_input_json: Set(tool_input),
        status: Set("pending".to_string()),
        denied_reason: Set(None),
        created_at: Set(created_at.into()),
        timeout_at: Set(timeout_at.into()),
        responded_at: Set(None),
        responded_by_client_id: Set(None),
        updated_at: Set(now.into()),
        ..Default::default()
    };

    let inserted = active.insert(db).await?;
    Approval::from_model(inserted)
}

pub async fn respond<C: ConnectionTrait>(
    db: &C,
    approval_id: Uuid,
    status: ApprovalStatus,
    responded_by_client_id: Option<String>,
) -> Result<Approval, DbErr> {
    let record = approval::Entity::find()
        .filter(approval::Column::Uuid.eq(approval_id))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Approval not found".to_string()))?;

    let now = Utc::now();
    let (status_raw, denied_reason) = status_to_db(&status);

    let mut active: approval::ActiveModel = record.into();
    active.status = Set(status_raw.to_string());
    active.denied_reason = Set(denied_reason);
    active.responded_at = Set(Some(now.into()));
    active.responded_by_client_id = Set(responded_by_client_id);
    active.updated_at = Set(now.into());

    let updated = active.update(db).await?;
    Approval::from_model(updated)
}

pub async fn list_by_attempt<C: ConnectionTrait>(
    db: &C,
    attempt_id: Uuid,
    status: Option<&str>,
    limit: u64,
    cursor: Option<i64>,
) -> Result<(Vec<Approval>, Option<i64>), DbErr> {
    let limit = limit.clamp(1, 200);
    let mut query = approval::Entity::find().filter(approval::Column::AttemptId.eq(attempt_id));
    if let Some(status) = status {
        query = query.filter(approval::Column::Status.eq(status));
    }
    if let Some(cursor) = cursor {
        query = query.filter(approval::Column::Id.lt(cursor));
    }

    let mut records = query
        .order_by_desc(approval::Column::Id)
        .limit(limit)
        .all(db)
        .await?;

    let next_cursor = records.last().map(|r| r.id);
    let approvals = records
        .drain(..)
        .map(Approval::from_model)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((approvals, next_cursor))
}
