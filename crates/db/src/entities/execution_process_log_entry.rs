use sea_orm::{JsonValue, entity::prelude::*};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "execution_process_log_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub execution_process_id: i64,
    pub channel: LogChannel,
    pub entry_index: i64,
    pub entry_json: JsonValue,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum LogChannel {
    #[sea_orm(string_value = "raw")]
    Raw,
    #[sea_orm(string_value = "normalized")]
    Normalized,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
