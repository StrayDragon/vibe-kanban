use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use ts_rs::TS;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
    EnumString,
    Display,
    Default,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum TaskStatus {
    #[default]
    #[sea_orm(string_value = "todo")]
    Todo,
    #[sea_orm(string_value = "inprogress")]
    InProgress,
    #[sea_orm(string_value = "inreview")]
    InReview,
    #[sea_orm(string_value = "done")]
    Done,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum ExecutionProcessStatus {
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "killed")]
    Killed,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
pub enum ExecutionProcessRunReason {
    #[sea_orm(string_value = "setupscript")]
    SetupScript,
    #[sea_orm(string_value = "cleanupscript")]
    CleanupScript,
    #[sea_orm(string_value = "codingagent")]
    CodingAgent,
    #[sea_orm(string_value = "devserver")]
    DevServer,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "merged")]
    Merged,
    #[sea_orm(string_value = "closed")]
    Closed,
    #[sea_orm(string_value = "unknown")]
    Unknown,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    TS,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum MergeType {
    #[sea_orm(string_value = "direct")]
    Direct,
    #[sea_orm(string_value = "pr")]
    Pr,
}
