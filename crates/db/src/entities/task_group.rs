use sea_orm::{JsonValue, entity::prelude::*};

use crate::types::{MilestoneAutomationMode, TaskStatus};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "task_groups")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub project_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub objective: Option<String>,
    pub definition_of_done: Option<String>,
    pub default_executor_profile_id: Option<JsonValue>,
    pub automation_mode: MilestoneAutomationMode,
    pub run_next_step_requested_at: Option<DateTimeUtc>,
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
