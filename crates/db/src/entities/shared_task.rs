use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "shared_tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub remote_project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_first_name: Option<String>,
    pub assignee_last_name: Option<String>,
    pub assignee_username: Option<String>,
    pub version: i32,
    pub last_event_seq: Option<i32>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
