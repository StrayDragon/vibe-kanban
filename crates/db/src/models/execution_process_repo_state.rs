use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{entities::execution_process_repo_state, models::ids};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExecutionProcessRepoState {
    pub id: Uuid,
    pub execution_process_id: Uuid,
    pub repo_id: Uuid,
    pub before_head_commit: Option<String>,
    pub after_head_commit: Option<String>,
    pub merge_commit: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateExecutionProcessRepoState {
    pub repo_id: Uuid,
    pub before_head_commit: Option<String>,
    pub after_head_commit: Option<String>,
    pub merge_commit: Option<String>,
}

impl ExecutionProcessRepoState {
    fn from_model(
        model: execution_process_repo_state::Model,
        execution_process_id: Uuid,
        repo_id: Uuid,
    ) -> Self {
        Self {
            id: model.uuid,
            execution_process_id,
            repo_id,
            before_head_commit: model.before_head_commit,
            after_head_commit: model.after_head_commit,
            merge_commit: model.merge_commit,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub async fn create_many<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
        entries: &[CreateExecutionProcessRepoState],
    ) -> Result<(), DbErr> {
        if entries.is_empty() {
            return Ok(());
        }

        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let now = Utc::now();
        let mut inserts = Vec::with_capacity(entries.len());
        for entry in entries {
            let repo_row_id = ids::repo_id_by_uuid(db, entry.repo_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            inserts.push(execution_process_repo_state::ActiveModel {
                uuid: Set(Uuid::new_v4()),
                execution_process_id: Set(execution_row_id),
                repo_id: Set(repo_row_id),
                before_head_commit: Set(entry.before_head_commit.clone()),
                after_head_commit: Set(entry.after_head_commit.clone()),
                merge_commit: Set(entry.merge_commit.clone()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
                ..Default::default()
            });
        }

        execution_process_repo_state::Entity::insert_many(inserts)
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn update_before_head_commit<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
        repo_id: Uuid,
        before_head_commit: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let record = execution_process_repo_state::Entity::find()
            .filter(execution_process_repo_state::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_repo_state::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo state not found".to_string()))?;

        let mut active: execution_process_repo_state::ActiveModel = record.into();
        active.before_head_commit = Set(Some(before_head_commit.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    pub async fn update_after_head_commit<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
        repo_id: Uuid,
        after_head_commit: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let record = execution_process_repo_state::Entity::find()
            .filter(execution_process_repo_state::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_repo_state::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo state not found".to_string()))?;

        let mut active: execution_process_repo_state::ActiveModel = record.into();
        active.after_head_commit = Set(Some(after_head_commit.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    pub async fn set_merge_commit<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
        repo_id: Uuid,
        merge_commit: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let repo_row_id = ids::repo_id_by_uuid(db, repo_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;

        let record = execution_process_repo_state::Entity::find()
            .filter(execution_process_repo_state::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_repo_state::Column::RepoId.eq(repo_row_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Repo state not found".to_string()))?;

        let mut active: execution_process_repo_state::ActiveModel = record.into();
        active.merge_commit = Set(Some(merge_commit.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    pub async fn find_by_execution_process_id<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let models = execution_process_repo_state::Entity::find()
            .filter(execution_process_repo_state::Column::ExecutionProcessId.eq(execution_row_id))
            .order_by_asc(execution_process_repo_state::Column::CreatedAt)
            .all(db)
            .await?;

        let mut states = Vec::with_capacity(models.len());
        for model in models {
            let repo_id = ids::repo_uuid_by_id(db, model.repo_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Repo not found".to_string()))?;
            states.push(Self::from_model(model, execution_process_id, repo_id));
        }
        Ok(states)
    }
}
