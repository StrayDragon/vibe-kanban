use sea_orm::entity::prelude::*;
use sea_orm::JsonValue;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "drafts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub session_id: i64,
    pub draft_type: String,
    pub retry_process_id: Option<i64>,
    pub prompt: String,
    pub queued: bool,
    pub sending: bool,
    pub version: i32,
    pub variant: Option<String>,
    pub image_ids: JsonValue,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
