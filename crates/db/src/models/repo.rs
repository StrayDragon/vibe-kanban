use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
    sea_query::{Expr, ExprTrait, Query},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use crate::entities::{project_repo, repo, workspace_repo};

#[derive(Debug, Error)]
pub enum RepoError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Repository not found")]
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Repo {
    pub id: Uuid,
    pub path: PathBuf,
    pub name: String,
    pub display_name: String,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

impl Repo {
    fn from_model(model: repo::Model) -> Self {
        Self {
            id: model.uuid,
            path: PathBuf::from(model.path),
            name: model.name,
            display_name: model.display_name,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    /// Get repos that still have the migration sentinel as their name.
    /// Used by the startup backfill to fix repo names.
    pub async fn list_needing_name_fix<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let records = repo::Entity::find()
            .filter(repo::Column::Name.eq("__NEEDS_BACKFILL__"))
            .all(db)
            .await?;
        Ok(records.into_iter().map(Self::from_model).collect())
    }

    pub async fn update_name<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        name: &str,
        display_name: &str,
    ) -> Result<(), DbErr> {
        let record = repo::Entity::find()
            .filter(repo::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let mut active: repo::ActiveModel = record.into();
        active.name = Set(name.to_string());
        active.display_name = Set(display_name.to_string());
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = repo::Entity::find()
            .filter(repo::Column::Uuid.eq(id))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn find_or_create<C: ConnectionTrait>(
        db: &C,
        path: &Path,
        display_name: &str,
    ) -> Result<Self, DbErr> {
        let path_str = path.to_string_lossy().to_string();
        if let Some(existing) = repo::Entity::find()
            .filter(repo::Column::Path.eq(path_str.clone()))
            .one(db)
            .await?
        {
            return Ok(Self::from_model(existing));
        }

        let id = Uuid::new_v4();
        let repo_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| id.to_string());
        let now = Utc::now();
        let active = repo::ActiveModel {
            uuid: Set(id),
            path: Set(path_str.clone()),
            name: Set(repo_name),
            display_name: Set(display_name.to_string()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let insert = repo::Entity::insert(active)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(repo::Column::Path)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(db)
            .await?;

        let created = if insert.last_insert_id > 0 {
            repo::Entity::find_by_id(insert.last_insert_id)
                .one(db)
                .await?
        } else {
            None
        };
        if let Some(created) = created {
            return Ok(Self::from_model(created));
        }

        let record = repo::Entity::find()
            .filter(repo::Column::Path.eq(path_str))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
        Ok(Self::from_model(record))
    }

    pub async fn delete_orphaned<C: ConnectionTrait>(db: &C) -> Result<u64, DbErr> {
        let delete = Query::delete()
            .from_table(repo::Entity)
            .and_where(
                Expr::col((repo::Entity, repo::Column::Id))
                    .not_in_subquery(
                        Query::select()
                            .column(project_repo::Column::RepoId)
                            .from(project_repo::Entity)
                            .to_owned(),
                    )
                    .and(
                        Expr::col((repo::Entity, repo::Column::Id)).not_in_subquery(
                            Query::select()
                                .column(workspace_repo::Column::RepoId)
                                .from(workspace_repo::Entity)
                                .to_owned(),
                        ),
                    ),
            )
            .to_owned();

        let result = db.execute(&delete).await?;
        Ok(result.rows_affected())
    }
}

impl From<repo::Model> for Repo {
    fn from(model: repo::Model) -> Self {
        Self::from_model(model)
    }
}
