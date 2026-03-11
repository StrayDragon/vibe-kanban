use sea_orm::entity::prelude::*;

use crate::types::TaskCreatedByKind;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "milestone_plan_applications")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub milestone_id: i64,
    pub schema_version: i32,
    pub plan_json: String,
    pub applied_by_kind: TaskCreatedByKind,
    pub idempotency_key: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

