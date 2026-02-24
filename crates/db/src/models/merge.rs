use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

pub use crate::types::{MergeStatus, MergeType};
use crate::{entities::merge, models::ids};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Merge {
    Direct(DirectMerge),
    Pr(PrMerge),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DirectMerge {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
    pub merge_commit: String,
    pub target_branch_name: String,
    pub created_at: DateTime<Utc>,
}

/// PR merge - represents a pull request merge
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PrMerge {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub target_branch_name: String,
    pub pr_info: PullRequestInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PullRequestInfo {
    pub number: i64,
    pub url: String,
    pub status: MergeStatus,
    pub merged_at: Option<DateTime<Utc>>,
    pub merge_commit_sha: Option<String>,
}

impl Merge {
    pub fn merge_commit(&self) -> Option<String> {
        match self {
            Merge::Direct(direct) => Some(direct.merge_commit.clone()),
            Merge::Pr(pr) => pr.pr_info.merge_commit_sha.clone(),
        }
    }

    async fn from_model<C: ConnectionTrait>(db: &C, model: merge::Model) -> Result<Self, DbErr> {
        let workspace_id = ids::workspace_uuid_by_id(db, model.workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let repo_id = ids::repo_uuid_by_id(db, model.repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        match model.merge_type {
            MergeType::Direct => Ok(Merge::Direct(DirectMerge {
                id: model.uuid,
                workspace_id,
                repo_id,
                merge_commit: model
                    .merge_commit
                    .expect("direct merge must have merge_commit"),
                target_branch_name: model.target_branch_name,
                created_at: model.created_at.into(),
            })),
            MergeType::Pr => Ok(Merge::Pr(PrMerge {
                id: model.uuid,
                workspace_id,
                repo_id,
                created_at: model.created_at.into(),
                target_branch_name: model.target_branch_name,
                pr_info: PullRequestInfo {
                    number: model.pr_number.expect("pr merge must have pr_number"),
                    url: model.pr_url.expect("pr merge must have pr_url"),
                    status: model.pr_status.expect("pr merge must have status"),
                    merged_at: model.pr_merged_at.map(Into::into),
                    merge_commit_sha: model.pr_merge_commit_sha,
                },
            })),
        }
    }

    /// Create a direct merge record
    pub async fn create_direct<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        repo_id: Uuid,
        target_branch_name: &str,
        merge_commit: &str,
    ) -> Result<DirectMerge, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
        let now = Utc::now();

        let active = merge::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            workspace_id: Set(workspace_row_id),
            repo_id: Set(repo_row_id),
            merge_type: Set(MergeType::Direct),
            merge_commit: Set(Some(merge_commit.to_string())),
            target_branch_name: Set(target_branch_name.to_string()),
            created_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        match Self::from_model(db, model).await? {
            Merge::Direct(direct) => Ok(direct),
            _ => Err(DbErr::Custom("Unexpected merge type".to_string())),
        }
    }

    /// Create a new PR record (when PR is opened)
    pub async fn create_pr<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        repo_id: Uuid,
        target_branch_name: &str,
        pr_number: i64,
        pr_url: &str,
    ) -> Result<PrMerge, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
        let now = Utc::now();

        let active = merge::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            workspace_id: Set(workspace_row_id),
            repo_id: Set(repo_row_id),
            merge_type: Set(MergeType::Pr),
            pr_number: Set(Some(pr_number)),
            pr_url: Set(Some(pr_url.to_string())),
            pr_status: Set(Some(MergeStatus::Open)),
            target_branch_name: Set(target_branch_name.to_string()),
            created_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        match Self::from_model(db, model).await? {
            Merge::Pr(pr) => Ok(pr),
            _ => Err(DbErr::Custom("Unexpected merge type".to_string())),
        }
    }

    /// Get all open PRs for monitoring
    pub async fn get_open_prs<C: ConnectionTrait>(db: &C) -> Result<Vec<PrMerge>, DbErr> {
        let records = merge::Entity::find()
            .filter(merge::Column::MergeType.eq(MergeType::Pr))
            .filter(merge::Column::PrStatus.eq(MergeStatus::Open))
            .order_by_desc(merge::Column::CreatedAt)
            .all(db)
            .await?;

        let mut merges = Vec::with_capacity(records.len());
        for model in records {
            if let Merge::Pr(pr) = Self::from_model(db, model).await? {
                merges.push(pr);
            }
        }
        Ok(merges)
    }

    /// Update PR status for a workspace
    pub async fn update_status<C: ConnectionTrait>(
        db: &C,
        merge_id: Uuid,
        pr_status: MergeStatus,
        merge_commit_sha: Option<String>,
    ) -> Result<(), DbErr> {
        let merged_at = if matches!(pr_status, MergeStatus::Merged) {
            Some(Utc::now())
        } else {
            None
        };

        let record = merge::Entity::find()
            .filter(merge::Column::Uuid.eq(merge_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Merge not found".to_string()))?;

        let mut active: merge::ActiveModel = record.into();
        active.pr_status = Set(Some(pr_status));
        active.pr_merge_commit_sha = Set(merge_commit_sha);
        active.pr_merged_at = Set(merged_at.map(Into::into));
        active.update(db).await?;
        Ok(())
    }

    /// Find all merges for a workspace (returns both direct and PR merges)
    pub async fn find_by_workspace_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let records = merge::Entity::find()
            .filter(merge::Column::WorkspaceId.eq(workspace_row_id))
            .order_by_desc(merge::Column::CreatedAt)
            .all(db)
            .await?;

        let mut merges = Vec::with_capacity(records.len());
        for model in records {
            merges.push(Self::from_model(db, model).await?);
        }
        Ok(merges)
    }

    /// Find all merges for a workspace and specific repo
    pub async fn find_by_workspace_and_repo_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let records = merge::Entity::find()
            .filter(merge::Column::WorkspaceId.eq(workspace_row_id))
            .filter(merge::Column::RepoId.eq(repo_row_id))
            .order_by_desc(merge::Column::CreatedAt)
            .all(db)
            .await?;

        let mut merges = Vec::with_capacity(records.len());
        for model in records {
            merges.push(Self::from_model(db, model).await?);
        }
        Ok(merges)
    }
}
