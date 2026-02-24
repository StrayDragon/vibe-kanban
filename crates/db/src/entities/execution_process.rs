use sea_orm::{JsonValue, entity::prelude::*};

use crate::types::{ExecutionProcessRunReason, ExecutionProcessStatus};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "execution_processes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub session_id: i64,
    pub run_reason: ExecutionProcessRunReason,
    pub executor_action: JsonValue,
    pub status: ExecutionProcessStatus,
    pub exit_code: Option<i64>,
    pub dropped: bool,
    pub started_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
