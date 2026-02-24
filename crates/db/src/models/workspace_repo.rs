use std::path::PathBuf;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QuerySelect,
    Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::repo::Repo;
use crate::{
    entities::{project_repo, repo, task, workspace, workspace_repo},
    models::ids,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WorkspaceRepo {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub repo_id: Uuid,
    pub target_branch: String,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateWorkspaceRepo {
    pub repo_id: Uuid,
    pub target_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RepoWithTargetBranch {
    #[serde(flatten)]
    pub repo: Repo,
    pub target_branch: String,
}

/// Repo info with copy_files configuration from project_repos.
#[derive(Debug, Clone)]
pub struct RepoWithCopyFiles {
    pub id: Uuid,
    pub path: PathBuf,
    pub name: String,
    pub copy_files: Option<String>,
}

impl WorkspaceRepo {
    fn from_model(model: workspace_repo::Model, workspace_id: Uuid, repo_id: Uuid) -> Self {
        Self {
            id: model.uuid,
            workspace_id,
            repo_id,
            target_branch: model.target_branch,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub async fn create_many<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        repos: &[CreateWorkspaceRepo],
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let mut results = Vec::with_capacity(repos.len());
        for repo in repos {
            let repo_row_id = ids::repo_id_by_uuid(db, repo.repo_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            let active = workspace_repo::ActiveModel {
                uuid: Set(Uuid::new_v4()),
                workspace_id: Set(workspace_row_id),
                repo_id: Set(repo_row_id),
                target_branch: Set(repo.target_branch.clone()),
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
                ..Default::default()
            };
            let model = active.insert(db).await?;
            results.push(Self::from_model(model, workspace_id, repo.repo_id));
        }
        Ok(results)
    }

    pub async fn find_by_workspace_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let models = workspace_repo::Entity::find()
            .filter(workspace_repo::Column::WorkspaceId.eq(workspace_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::with_capacity(models.len());
        for model in models {
            let repo_id = ids::repo_uuid_by_id(db, model.repo_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            repos.push(Self::from_model(model, workspace_id, repo_id));
        }
        Ok(repos)
    }

    pub async fn find_repos_for_workspace<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<Repo>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let models = workspace_repo::Entity::find()
            .filter(workspace_repo::Column::WorkspaceId.eq(workspace_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::new();
        for model in models {
            if let Some(repo_model) = repo::Entity::find_by_id(model.repo_id).one(db).await? {
                repos.push(Repo::from(repo_model));
            }
        }
        repos.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        Ok(repos)
    }

    pub async fn find_repos_with_target_branch_for_workspace<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<RepoWithTargetBranch>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let models = workspace_repo::Entity::find()
            .filter(workspace_repo::Column::WorkspaceId.eq(workspace_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::with_capacity(models.len());
        for model in models {
            let repo_model = repo::Entity::find_by_id(model.repo_id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            repos.push(RepoWithTargetBranch {
                repo: Repo::from(repo_model),
                target_branch: model.target_branch,
            });
        }
        repos.sort_by(|a, b| a.repo.display_name.cmp(&b.repo.display_name));
        Ok(repos)
    }

    pub async fn find_by_workspace_and_repo_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let record = workspace_repo::Entity::find()
            .filter(workspace_repo::Column::WorkspaceId.eq(workspace_row_id))
            .filter(workspace_repo::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?;

        Ok(record.map(|model| Self::from_model(model, workspace_id, repo_id)))
    }

    pub async fn update_target_branch<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
        repo_id: Uuid,
        new_target_branch: &str,
    ) -> Result<(), DbErr> {
        let record = Self::find_by_workspace_and_repo_id(db, workspace_id, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Workspace repo not found".to_string(),
            ))?;

        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let model = workspace_repo::Entity::find()
            .filter(workspace_repo::Column::WorkspaceId.eq(workspace_row_id))
            .filter(workspace_repo::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Workspace repo not found".to_string(),
            ))?;

        let mut active: workspace_repo::ActiveModel = model.into();
        active.target_branch = Set(new_target_branch.to_string());
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;

        let _ = record;
        Ok(())
    }

    pub async fn update_target_branch_for_children_of_workspace<C: ConnectionTrait>(
        db: &C,
        parent_workspace_id: Uuid,
        old_branch: &str,
        new_branch: &str,
    ) -> Result<u64, DbErr> {
        let parent_workspace_row_id =
            match ids::workspace_id_by_uuid(db, parent_workspace_id).await? {
                Some(id) => id,
                None => return Ok(0),
            };

        let task_ids: Vec<i64> = task::Entity::find()
            .select_only()
            .column(task::Column::Id)
            .filter(task::Column::ParentWorkspaceId.eq(parent_workspace_row_id))
            .into_tuple()
            .all(db)
            .await?;

        if task_ids.is_empty() {
            return Ok(0);
        }

        let workspace_ids: Vec<i64> = workspace::Entity::find()
            .select_only()
            .column(workspace::Column::Id)
            .filter(workspace::Column::TaskId.is_in(task_ids))
            .into_tuple()
            .all(db)
            .await?;

        if workspace_ids.is_empty() {
            return Ok(0);
        }

        let result = workspace_repo::Entity::update_many()
            .col_expr(
                workspace_repo::Column::TargetBranch,
                Expr::value(new_branch.to_string()),
            )
            .filter(workspace_repo::Column::TargetBranch.eq(old_branch))
            .filter(workspace_repo::Column::WorkspaceId.is_in(workspace_ids))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    pub async fn find_unique_repos_for_task<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
    ) -> Result<Vec<Repo>, DbErr> {
        let task_row_id = match ids::task_id_by_uuid(db, task_id).await? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let workspace_ids: Vec<i64> = workspace::Entity::find()
            .select_only()
            .column(workspace::Column::Id)
            .filter(workspace::Column::TaskId.eq(task_row_id))
            .into_tuple()
            .all(db)
            .await?;

        if workspace_ids.is_empty() {
            return Ok(Vec::new());
        }

        let repo_ids: Vec<i64> = workspace_repo::Entity::find()
            .select_only()
            .column(workspace_repo::Column::RepoId)
            .filter(workspace_repo::Column::WorkspaceId.is_in(workspace_ids))
            .into_tuple()
            .all(db)
            .await?;

        let mut repos = Vec::new();
        for repo_id in repo_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
        {
            if let Some(repo_model) = repo::Entity::find_by_id(repo_id).one(db).await? {
                repos.push(Repo::from(repo_model));
            }
        }
        repos.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        Ok(repos)
    }

    /// Find repos for a workspace with their copy_files configuration.
    /// Uses LEFT JOIN so repos without project_repo entries still appear (with NULL copy_files).
    pub async fn find_repos_with_copy_files<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<RepoWithCopyFiles>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let workspace_model = workspace::Entity::find_by_id(workspace_row_id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let task_model = task::Entity::find_by_id(workspace_model.task_id)
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let models = workspace_repo::Entity::find()
            .filter(workspace_repo::Column::WorkspaceId.eq(workspace_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::with_capacity(models.len());
        for model in models {
            let repo_model = repo::Entity::find_by_id(model.repo_id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            let copy_files = project_repo::Entity::find()
                .filter(project_repo::Column::ProjectId.eq(task_model.project_id))
                .filter(project_repo::Column::RepoId.eq(repo_model.id))
                .one(db)
                .await?
                .and_then(|row| row.copy_files);
            repos.push(RepoWithCopyFiles {
                id: repo_model.uuid,
                path: PathBuf::from(repo_model.path),
                name: repo_model.name,
                copy_files,
            });
        }

        Ok(repos)
    }
}
