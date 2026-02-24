use sea_orm::{JsonValue, entity::prelude::*};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "scratch")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub session_id: i64,
    pub scratch_type: String,
    pub payload: JsonValue,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
