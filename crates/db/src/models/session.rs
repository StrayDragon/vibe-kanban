use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use crate::{entities::session, models::ids};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Session not found")]
    NotFound,
    #[error("Workspace not found")]
    WorkspaceNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Session {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub executor: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateSession {
    pub executor: Option<String>,
}

impl Session {
    fn from_model(model: session::Model, workspace_id: Uuid) -> Self {
        Self {
            id: model.uuid,
            workspace_id,
            executor: model.executor,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub async fn find_by_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let record = session::Entity::find()
            .filter(session::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => {
                let workspace_uuid = ids::workspace_uuid_by_id(db, model.workspace_id)
                    .await?
                    .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
                Ok(Some(Self::from_model(model, workspace_uuid)))
            }
            None => Ok(None),
        }
    }

    pub async fn find_by_workspace_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let records = session::Entity::find()
            .filter(session::Column::WorkspaceId.eq(workspace_row_id))
            .order_by_desc(session::Column::CreatedAt)
            .all(db)
            .await?;

        Ok(records
            .into_iter()
            .map(|model| Self::from_model(model, workspace_id))
            .collect())
    }

    /// Find the latest session for a workspace
    pub async fn find_latest_by_workspace_id<C: ConnectionTrait>(
        db: &C,
        workspace_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;

        let record = session::Entity::find()
            .filter(session::Column::WorkspaceId.eq(workspace_row_id))
            .order_by_desc(session::Column::CreatedAt)
            .one(db)
            .await?;

        Ok(record.map(|model| Self::from_model(model, workspace_id)))
    }

    pub async fn find_latest_by_workspace_ids<C: ConnectionTrait>(
        db: &C,
        workspace_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Session>, DbErr> {
        if workspace_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut workspace_map = HashMap::new();
        let mut row_ids = Vec::new();
        for workspace_id in workspace_ids {
            if let Some(id) = ids::workspace_id_by_uuid(db, *workspace_id).await? {
                workspace_map.insert(id, *workspace_id);
                row_ids.push(id);
            }
        }

        if row_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let records = session::Entity::find()
            .filter(session::Column::WorkspaceId.is_in(row_ids))
            .order_by_asc(session::Column::WorkspaceId)
            .order_by_desc(session::Column::CreatedAt)
            .all(db)
            .await?;

        let mut latest_by_workspace = HashMap::new();
        for model in records {
            if let Some(workspace_id) = workspace_map.get(&model.workspace_id) {
                latest_by_workspace
                    .entry(*workspace_id)
                    .or_insert_with(|| Self::from_model(model, *workspace_id));
            }
        }

        Ok(latest_by_workspace)
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateSession,
        id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Self, SessionError> {
        let workspace_row_id = ids::workspace_id_by_uuid(db, workspace_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Workspace not found".to_string()))?;
        let now = Utc::now();
        let active = session::ActiveModel {
            uuid: Set(id),
            workspace_id: Set(workspace_row_id),
            executor: Set(data.executor.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        Ok(Self::from_model(model, workspace_id))
    }
}
