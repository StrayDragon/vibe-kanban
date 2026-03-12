use sea_orm::entity::prelude::*;

use crate::types::{TaskContinuationStopReasonCode, TaskControlTransferReasonCode, VkNextAction};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "task_orchestration_states")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub task_id: i64,
    pub attempt_id: Option<Uuid>,
    pub continuation_turns_used: i32,
    pub last_vk_next_action: Option<VkNextAction>,
    pub last_vk_next_invalid_raw: Option<String>,
    pub last_vk_next_at: Option<DateTimeUtc>,
    pub last_continuation_stop_reason_code: Option<TaskContinuationStopReasonCode>,
    pub last_continuation_stop_reason_detail: Option<String>,
    pub last_continuation_stop_at: Option<DateTimeUtc>,
    pub last_control_transfer_reason_code: Option<TaskControlTransferReasonCode>,
    pub last_control_transfer_detail: Option<String>,
    pub last_control_transfer_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
