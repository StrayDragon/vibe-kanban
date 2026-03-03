use sea_orm::{JsonValue, entity::prelude::*};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "mcp_tool_tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub task_id: String,
    pub created_by_client_id: Option<String>,
    pub tool_name: String,
    pub tool_arguments_json: JsonValue,
    pub status: String,
    pub status_message: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub kanban_task_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub resumable: bool,
    pub ttl_ms: Option<i64>,
    pub poll_interval_ms: Option<i64>,
    pub result_json: Option<JsonValue>,
    pub error_json: Option<JsonValue>,
    pub created_at: DateTimeUtc,
    pub last_updated_at: DateTimeUtc,
    pub expires_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
