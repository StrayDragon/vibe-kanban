use sea_orm::entity::prelude::*;

use crate::types::{MergeStatus, MergeType};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "merges")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub uuid: Uuid,
    pub workspace_id: i64,
    pub repo_id: i64,
    pub merge_type: MergeType,
    pub merge_commit: Option<String>,
    pub target_branch_name: String,
    pub pr_number: Option<i64>,
    pub pr_url: Option<String>,
    pub pr_status: Option<MergeStatus>,
    pub pr_merged_at: Option<DateTimeUtc>,
    pub pr_merge_commit_sha: Option<String>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
