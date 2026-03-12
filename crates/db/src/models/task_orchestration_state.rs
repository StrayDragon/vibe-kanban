use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

use crate::{
    entities::task_orchestration_state,
    models::ids,
    types::{TaskContinuationStopReasonCode, TaskControlTransferReasonCode, VkNextAction},
};

#[derive(Debug, Clone)]
pub struct TaskOrchestrationState {
    pub task_id: Uuid,
    pub attempt_id: Option<Uuid>,
    pub continuation_turns_used: i32,
    pub last_vk_next_action: Option<VkNextAction>,
    pub last_vk_next_invalid_raw: Option<String>,
    pub last_vk_next_at: Option<DateTime<Utc>>,
    pub last_continuation_stop_reason_code: Option<TaskContinuationStopReasonCode>,
    pub last_continuation_stop_reason_detail: Option<String>,
    pub last_continuation_stop_at: Option<DateTime<Utc>>,
    pub last_control_transfer_reason_code: Option<TaskControlTransferReasonCode>,
    pub last_control_transfer_detail: Option<String>,
    pub last_control_transfer_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskOrchestrationState {
    fn from_model(model: task_orchestration_state::Model, task_id: Uuid) -> Self {
        Self {
            task_id,
            attempt_id: model.attempt_id,
            continuation_turns_used: model.continuation_turns_used,
            last_vk_next_action: model.last_vk_next_action,
            last_vk_next_invalid_raw: model.last_vk_next_invalid_raw,
            last_vk_next_at: model.last_vk_next_at.map(Into::into),
            last_continuation_stop_reason_code: model.last_continuation_stop_reason_code,
            last_continuation_stop_reason_detail: model.last_continuation_stop_reason_detail,
            last_continuation_stop_at: model.last_continuation_stop_at.map(Into::into),
            last_control_transfer_reason_code: model.last_control_transfer_reason_code,
            last_control_transfer_detail: model.last_control_transfer_detail,
            last_control_transfer_at: model.last_control_transfer_at.map(Into::into),
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

        let record = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        Ok(record.map(|model| Self::from_model(model, task_id)))
    }

    pub async fn upsert_vk_next_action<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        attempt_id: Uuid,
        action: Option<VkNextAction>,
        invalid_raw: Option<String>,
    ) -> Result<Self, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let now = Utc::now();
        let existing = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let model = if let Some(existing) = existing {
            let current_attempt = existing.attempt_id;
            let mut active: task_orchestration_state::ActiveModel = existing.into();
            if current_attempt != Some(attempt_id) {
                // New attempt: reset continuation counters and stop reason diagnostics.
                active.continuation_turns_used = Set(0);
                active.last_continuation_stop_reason_code = Set(None);
                active.last_continuation_stop_reason_detail = Set(None);
                active.last_continuation_stop_at = Set(None);
            }
            active.attempt_id = Set(Some(attempt_id));
            active.last_vk_next_action = Set(action);
            active.last_vk_next_invalid_raw = Set(invalid_raw);
            active.last_vk_next_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(db).await?
        } else {
            let active = task_orchestration_state::ActiveModel {
                task_id: Set(task_row_id),
                attempt_id: Set(Some(attempt_id)),
                continuation_turns_used: Set(0),
                last_vk_next_action: Set(action),
                last_vk_next_invalid_raw: Set(invalid_raw),
                last_vk_next_at: Set(Some(now.into())),
                last_continuation_stop_reason_code: Set(None),
                last_continuation_stop_reason_detail: Set(None),
                last_continuation_stop_at: Set(None),
                last_control_transfer_reason_code: Set(None),
                last_control_transfer_detail: Set(None),
                last_control_transfer_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };
            active.insert(db).await?
        };

        Ok(Self::from_model(model, task_id))
    }

    pub async fn increment_continuation_turns_used<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        attempt_id: Uuid,
    ) -> Result<Self, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let now = Utc::now();
        let existing = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let model = if let Some(existing) = existing {
            let current_attempt = existing.attempt_id;
            let current_used = existing.continuation_turns_used;
            let next_used = if current_attempt == Some(attempt_id) {
                current_used + 1
            } else {
                1
            };

            let mut active: task_orchestration_state::ActiveModel = existing.into();
            active.attempt_id = Set(Some(attempt_id));
            active.continuation_turns_used = Set(next_used.max(0));
            // Clear stop reason on successful continuation start.
            active.last_continuation_stop_reason_code = Set(None);
            active.last_continuation_stop_reason_detail = Set(None);
            active.last_continuation_stop_at = Set(None);
            active.updated_at = Set(now.into());
            active.update(db).await?
        } else {
            let active = task_orchestration_state::ActiveModel {
                task_id: Set(task_row_id),
                attempt_id: Set(Some(attempt_id)),
                continuation_turns_used: Set(1),
                last_vk_next_action: Set(None),
                last_vk_next_at: Set(None),
                last_continuation_stop_reason_code: Set(None),
                last_continuation_stop_reason_detail: Set(None),
                last_continuation_stop_at: Set(None),
                last_control_transfer_reason_code: Set(None),
                last_control_transfer_detail: Set(None),
                last_control_transfer_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };
            active.insert(db).await?
        };

        Ok(Self::from_model(model, task_id))
    }

    pub async fn decrement_continuation_turns_used_if_attempt_matches<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        attempt_id: Uuid,
    ) -> Result<(), DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let existing = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let Some(existing) = existing else {
            return Ok(());
        };
        if existing.attempt_id != Some(attempt_id) {
            return Ok(());
        }

        let now = Utc::now();
        let current_used = existing.continuation_turns_used;
        let next_used = (current_used - 1).max(0);

        let mut active: task_orchestration_state::ActiveModel = existing.into();
        active.continuation_turns_used = Set(next_used);
        active.updated_at = Set(now.into());
        active.update(db).await?;

        Ok(())
    }

    pub async fn record_continuation_stop_reason<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        attempt_id: Uuid,
        code: TaskContinuationStopReasonCode,
        detail: Option<String>,
    ) -> Result<Self, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let now = Utc::now();
        let existing = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let model = if let Some(existing) = existing {
            let current_attempt = existing.attempt_id;
            let mut active: task_orchestration_state::ActiveModel = existing.into();
            if current_attempt != Some(attempt_id) {
                // New attempt: reset continuation counters.
                active.continuation_turns_used = Set(0);
            }
            active.attempt_id = Set(Some(attempt_id));
            active.last_continuation_stop_reason_code = Set(Some(code));
            active.last_continuation_stop_reason_detail = Set(detail);
            active.last_continuation_stop_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(db).await?
        } else {
            let active = task_orchestration_state::ActiveModel {
                task_id: Set(task_row_id),
                attempt_id: Set(Some(attempt_id)),
                continuation_turns_used: Set(0),
                last_vk_next_action: Set(None),
                last_vk_next_at: Set(None),
                last_continuation_stop_reason_code: Set(Some(code)),
                last_continuation_stop_reason_detail: Set(detail),
                last_continuation_stop_at: Set(Some(now.into())),
                last_control_transfer_reason_code: Set(None),
                last_control_transfer_detail: Set(None),
                last_control_transfer_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };
            active.insert(db).await?
        };

        Ok(Self::from_model(model, task_id))
    }

    pub async fn record_control_transfer<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        reason: TaskControlTransferReasonCode,
        detail: Option<String>,
    ) -> Result<Self, DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let now = Utc::now();
        let existing = task_orchestration_state::Entity::find()
            .filter(task_orchestration_state::Column::TaskId.eq(task_row_id))
            .one(db)
            .await?;

        let model = if let Some(existing) = existing {
            let mut active: task_orchestration_state::ActiveModel = existing.into();
            active.last_control_transfer_reason_code = Set(Some(reason));
            active.last_control_transfer_detail = Set(detail);
            active.last_control_transfer_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(db).await?
        } else {
            let active = task_orchestration_state::ActiveModel {
                task_id: Set(task_row_id),
                attempt_id: Set(None),
                continuation_turns_used: Set(0),
                last_vk_next_action: Set(None),
                last_vk_next_at: Set(None),
                last_continuation_stop_reason_code: Set(None),
                last_continuation_stop_reason_detail: Set(None),
                last_continuation_stop_at: Set(None),
                last_control_transfer_reason_code: Set(Some(reason)),
                last_control_transfer_detail: Set(detail),
                last_control_transfer_at: Set(Some(now.into())),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            };
            active.insert(db).await?
        };

        Ok(Self::from_model(model, task_id))
    }
}
