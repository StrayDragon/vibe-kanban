use std::path::Path;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use super::repo::Repo;
use crate::{
    entities::{project_repo, repo},
    models::ids,
};

#[derive(Debug, Error)]
pub enum ProjectRepoError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Repository not found")]
    NotFound,
    #[error("Repository already exists in this project")]
    AlreadyExists,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ProjectRepo {
    pub id: Uuid,
    pub project_id: Uuid,
    pub repo_id: Uuid,
    pub setup_script: Option<String>,
    pub cleanup_script: Option<String>,
    pub copy_files: Option<String>,
    pub parallel_setup_script: bool,
}

/// ProjectRepo with the associated repo name (for script execution in worktrees)
#[derive(Debug, Clone)]
pub struct ProjectRepoWithName {
    pub id: Uuid,
    pub project_id: Uuid,
    pub repo_id: Uuid,
    pub repo_name: String,
    pub setup_script: Option<String>,
    pub cleanup_script: Option<String>,
    pub copy_files: Option<String>,
    pub parallel_setup_script: bool,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateProjectRepo {
    pub display_name: String,
    pub git_repo_path: String,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProjectRepo {
    pub setup_script: Option<String>,
    pub cleanup_script: Option<String>,
    pub copy_files: Option<String>,
    pub parallel_setup_script: Option<bool>,
}

impl ProjectRepo {
    fn from_model(model: project_repo::Model, project_id: Uuid, repo_id: Uuid) -> Self {
        Self {
            id: model.uuid,
            project_id,
            repo_id,
            setup_script: model.setup_script,
            cleanup_script: model.cleanup_script,
            copy_files: model.copy_files,
            parallel_setup_script: model.parallel_setup_script,
        }
    }

    pub async fn find_by_project_id<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let models = project_repo::Entity::find()
            .filter(project_repo::Column::ProjectId.eq(project_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::with_capacity(models.len());
        for model in models {
            let repo_id = ids::repo_uuid_by_id(db, model.repo_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            repos.push(Self::from_model(model, project_id, repo_id));
        }
        Ok(repos)
    }

    pub async fn find_by_repo_id<C: ConnectionTrait>(
        db: &C,
        repo_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let models = project_repo::Entity::find()
            .filter(project_repo::Column::RepoId.eq(repo_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::with_capacity(models.len());
        for model in models {
            let project_id = ids::project_uuid_by_id(db, model.project_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
            repos.push(Self::from_model(model, project_id, repo_id));
        }
        Ok(repos)
    }

    pub async fn find_by_project_id_with_names<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<ProjectRepoWithName>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let models = project_repo::Entity::find()
            .filter(project_repo::Column::ProjectId.eq(project_row_id))
            .all(db)
            .await?;

        let mut repos = Vec::with_capacity(models.len());
        for model in models {
            let repo_model = repo::Entity::find_by_id(model.repo_id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            repos.push(ProjectRepoWithName {
                id: model.uuid,
                project_id,
                repo_id: repo_model.uuid,
                repo_name: repo_model.name,
                setup_script: model.setup_script,
                cleanup_script: model.cleanup_script,
                copy_files: model.copy_files,
                parallel_setup_script: model.parallel_setup_script,
            });
        }

        repos.sort_by(|a, b| a.repo_name.cmp(&b.repo_name));
        Ok(repos)
    }

    pub async fn find_repos_for_project<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<Repo>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let models = project_repo::Entity::find()
            .filter(project_repo::Column::ProjectId.eq(project_row_id))
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

    pub async fn find_by_project_and_repo<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let record = project_repo::Entity::find()
            .filter(project_repo::Column::ProjectId.eq(project_row_id))
            .filter(project_repo::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?;

        Ok(record.map(|model| Self::from_model(model, project_id, repo_id)))
    }

    pub async fn add_repo_to_project<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
        repo_path: &str,
        repo_name: &str,
    ) -> Result<Repo, ProjectRepoError> {
        let repo = Repo::find_or_create(db, Path::new(repo_path), repo_name).await?;

        if Self::find_by_project_and_repo(db, project_id, repo.id)
            .await?
            .is_some()
        {
            return Err(ProjectRepoError::AlreadyExists);
        }

        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo.id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let active = project_repo::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            project_id: Set(project_row_id),
            repo_id: Set(repo_row_id),
            setup_script: Set(None),
            cleanup_script: Set(None),
            copy_files: Set(None),
            parallel_setup_script: Set(false),
            ..Default::default()
        };
        active.insert(db).await?;

        Ok(repo)
    }

    pub async fn remove_repo_from_project<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
        repo_id: Uuid,
    ) -> Result<(), ProjectRepoError> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let result = project_repo::Entity::delete_many()
            .filter(project_repo::Column::ProjectId.eq(project_row_id))
            .filter(project_repo::Column::RepoId.eq(repo_row_id))
            .exec(db)
            .await?;

        if result.rows_affected == 0 {
            return Err(ProjectRepoError::NotFound);
        }

        Ok(())
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Self, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let active = project_repo::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            project_id: Set(project_row_id),
            repo_id: Set(repo_row_id),
            setup_script: Set(None),
            cleanup_script: Set(None),
            copy_files: Set(None),
            parallel_setup_script: Set(false),
            ..Default::default()
        };
        let model = active.insert(db).await?;
        Ok(Self::from_model(model, project_id, repo_id))
    }

    pub async fn update<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
        repo_id: Uuid,
        payload: &UpdateProjectRepo,
    ) -> Result<Self, ProjectRepoError> {
        let record = Self::find_by_project_and_repo(db, project_id, repo_id)
            .await?
            .ok_or(ProjectRepoError::NotFound)?;

        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let model = project_repo::Entity::find()
            .filter(project_repo::Column::ProjectId.eq(project_row_id))
            .filter(project_repo::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?
            .ok_or(ProjectRepoError::NotFound)?;

        let mut active: project_repo::ActiveModel = model.into();
        if payload.setup_script.is_some() {
            active.setup_script = Set(payload.setup_script.clone());
        }
        if payload.cleanup_script.is_some() {
            active.cleanup_script = Set(payload.cleanup_script.clone());
        }
        if payload.copy_files.is_some() {
            active.copy_files = Set(payload.copy_files.clone());
        }
        if let Some(parallel) = payload.parallel_setup_script {
            active.parallel_setup_script = Set(parallel);
        }
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;
        Ok(Self::from_model(updated, record.project_id, record.repo_id))
    }
}
