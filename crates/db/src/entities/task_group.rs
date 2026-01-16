use sea_orm::entity::prelude::*;
use sea_orm::JsonValue;

use crate::types::TaskStatus;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "task_groups")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub baseline_ref: String,
    pub schema_version: i32,
    pub graph_json: JsonValue,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
