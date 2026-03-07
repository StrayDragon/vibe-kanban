use sea_orm::entity::prelude::*;

use crate::types::{TaskDispatchController, TaskDispatchStatus};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "task_dispatch_states")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub task_id: i64,
    pub controller: TaskDispatchController,
    pub status: TaskDispatchStatus,
    pub retry_count: i32,
    pub max_retries: i32,
    pub last_error: Option<String>,
    pub blocked_reason: Option<String>,
    pub next_retry_at: Option<DateTimeUtc>,
    pub claim_expires_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
