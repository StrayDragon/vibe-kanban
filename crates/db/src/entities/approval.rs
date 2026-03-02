use sea_orm::{JsonValue, entity::prelude::*};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "approvals")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub attempt_id: Uuid,
    pub execution_process_id: Uuid,
    pub tool_call_id: String,
    pub tool_name: String,
    pub tool_input_json: JsonValue,
    pub status: String,
    pub denied_reason: Option<String>,
    pub created_at: DateTimeUtc,
    pub timeout_at: DateTimeUtc,
    pub responded_at: Option<DateTimeUtc>,
    pub responded_by_client_id: Option<String>,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
