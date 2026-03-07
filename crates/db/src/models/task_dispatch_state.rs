use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{task, task_dispatch_state},
    events::{EVENT_TASK_UPDATED, TaskEventPayload},
    models::{event_outbox::EventOutbox, ids},
    types::{TaskDispatchController, TaskDispatchStatus},
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskDispatchState {
    pub task_id: Uuid,
    pub controller: TaskDispatchController,
    pub status: TaskDispatchStatus,
    pub retry_count: i32,
    pub max_retries: i32,
    pub last_error: Option<String>,
    pub blocked_reason: Option<String>,
    #[ts(type = "Date | null")]
    pub next_retry_at: Option<DateTime<Utc>>,
    #[ts(type = "Date | null")]
    pub claim_expires_at: Option<DateTime<Utc>>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertTaskDispatchState {
    pub controller: TaskDispatchController,
    pub status: TaskDispatchStatus,
    pub retry_count: i32,
    pub max_retries: i32,
    pub last_error: Option<String>,
    pub blocked_reason: Option<String>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub claim_expires_at: Option<DateTime<Utc>>,
}

impl TaskDispatchState {
    fn from_model(model: task_dispatch_state::Model, task_id: Uuid) -> Self {
        Self {
            task_id,
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
        }
    }

    pub async fn find_by_task_id<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let record = task_dispatch_state::Entity::find()
            .filter(task_dispatch_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        Ok(record.map(|model| Self::from_model(model, task_id)))
    }

    pub async fn upsert<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        data: &UpsertTaskDispatchState,
    ) -> Result<Self, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        let project_id = task::Entity::find_by_id(task_row_id)
            .one(db)
            .await?
            .map(|record| record.project_id)
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let now = Utc::now();
        let existing = task_dispatch_state::Entity::find()
            .filter(task_dispatch_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let model = if let Some(existing) = existing {
            let mut active: task_dispatch_state::ActiveModel = existing.into();
            active.controller = Set(data.controller.clone());
            active.status = Set(data.status.clone());
            active.retry_count = Set(data.retry_count.max(0));
            active.max_retries = Set(data.max_retries.max(0));
            active.last_error = Set(data.last_error.clone());
            active.blocked_reason = Set(data.blocked_reason.clone());
            active.next_retry_at = Set(data.next_retry_at.map(Into::into));
            active.claim_expires_at = Set(data.claim_expires_at.map(Into::into));
            active.updated_at = Set(now.into());
            active.update(db).await?
        } else {
            let active = task_dispatch_state::ActiveModel {
                task_id: Set(task_row_id),
                controller: Set(data.controller.clone()),
                status: Set(data.status.clone()),
                retry_count: Set(data.retry_count.max(0)),
                max_retries: Set(data.max_retries.max(0)),
                last_error: Set(data.last_error.clone()),
                blocked_reason: Set(data.blocked_reason.clone()),
                next_retry_at: Set(data.next_retry_at.map(Into::into)),
                claim_expires_at: Set(data.claim_expires_at.map(Into::into)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };
            active.insert(db).await?
        };

        let payload = serde_json::to_value(TaskEventPayload {
            task_id,
            project_id: ids::project_uuid_by_id(db, project_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", task_id, payload).await?;

        Ok(Self::from_model(model, task_id))
    }

    pub async fn delete_by_task_id<C: ConnectionTrait>(db: &C, task_id: Uuid) -> Result<(), DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
        task_dispatch_state::Entity::delete_many()
            .filter(task_dispatch_state::Column::TaskId.eq(task_row_id))
            .exec(db)
            .await?;
        Ok(())
    }
}
