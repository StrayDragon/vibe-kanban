use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumDiscriminants, EnumString};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::scratch,
    events::{
        EVENT_SCRATCH_CREATED, EVENT_SCRATCH_DELETED, EVENT_SCRATCH_UPDATED, ScratchEventPayload,
    },
    models::{event_outbox::EventOutbox, ids},
};

#[derive(Debug, Error)]
pub enum ScratchError {
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Scratch type mismatch: expected '{expected}' but got '{actual}'")]
    TypeMismatch { expected: String, actual: String },
}

/// Data for a draft follow-up scratch
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DraftFollowUpData {
    pub message: String,
    #[serde(default)]
    pub variant: Option<String>,
}

/// The payload of a scratch, tagged by type. The type is part of the composite primary key.
/// Data is stored as markdown string.
#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumDiscriminants)]
#[serde(tag = "type", content = "data", rename_all = "SCREAMING_SNAKE_CASE")]
#[strum_discriminants(name(ScratchType))]
#[strum_discriminants(derive(Display, EnumString, Serialize, Deserialize, TS))]
#[strum_discriminants(ts(use_ts_enum))]
#[strum_discriminants(serde(rename_all = "SCREAMING_SNAKE_CASE"))]
#[strum_discriminants(strum(serialize_all = "SCREAMING_SNAKE_CASE"))]
pub enum ScratchPayload {
    DraftTask(String),
    DraftFollowUp(DraftFollowUpData),
}

impl ScratchPayload {
    /// Returns the scratch type for this payload
    pub fn scratch_type(&self) -> ScratchType {
        ScratchType::from(self)
    }

    /// Validates that the payload type matches the expected type
    pub fn validate_type(&self, expected: ScratchType) -> Result<(), ScratchError> {
        let actual = self.scratch_type();
        if actual != expected {
            return Err(ScratchError::TypeMismatch {
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Scratch {
    pub id: Uuid,
    pub payload: ScratchPayload,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Scratch {
    /// Returns the scratch type derived from the payload
    pub fn scratch_type(&self) -> ScratchType {
        self.payload.scratch_type()
    }
}

fn map_row(model: scratch::Model, session_id: Uuid) -> Result<Scratch, ScratchError> {
    let payload: ScratchPayload = serde_json::from_value(model.payload)?;
    payload.validate_type(model.scratch_type.parse().map_err(|_| {
        ScratchError::TypeMismatch {
            expected: model.scratch_type.clone(),
            actual: payload.scratch_type().to_string(),
        }
    })?)?;

    Ok(Scratch {
        id: session_id,
        payload,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    })
}

/// Request body for creating a scratch (id comes from URL path, type from payload)
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CreateScratch {
    pub payload: ScratchPayload,
}

/// Request body for updating a scratch
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct UpdateScratch {
    pub payload: ScratchPayload,
}

impl Scratch {
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        data: &CreateScratch,
    ) -> Result<Self, ScratchError> {
        let scratch_type_str = data.payload.scratch_type().to_string();
        let payload_value = serde_json::to_value(&data.payload)?;

        let session_row_id = ids::session_id_by_uuid(db, id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let active = scratch::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            session_id: Set(session_row_id),
            scratch_type: Set(scratch_type_str.clone()),
            payload: Set(payload_value),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        let payload = serde_json::to_value(ScratchEventPayload {
            scratch_id: id,
            scratch_type: scratch_type_str.clone(),
        })
        .map_err(ScratchError::Serde)?;
        EventOutbox::enqueue(db, EVENT_SCRATCH_CREATED, "scratch", id, payload).await?;
        map_row(model, id)
    }

    pub async fn find_by_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        scratch_type: &ScratchType,
    ) -> Result<Option<Self>, ScratchError> {
        let scratch_type_str = scratch_type.to_string();
        let session_row_id = ids::session_id_by_uuid(db, id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let record = scratch::Entity::find()
            .filter(scratch::Column::SessionId.eq(session_row_id))
            .filter(scratch::Column::ScratchType.eq(scratch_type_str))
            .one(db)
            .await?;

        let scratch = record.map(|row| map_row(row, id)).transpose()?;
        Ok(scratch)
    }

    pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, ScratchError> {
        let records = scratch::Entity::find()
            .order_by_desc(scratch::Column::CreatedAt)
            .all(db)
            .await?;

        let mut scratches = Vec::with_capacity(records.len());
        for record in records {
            let scratch = match ids::session_uuid_by_id(db, record.session_id).await? {
                Some(session_id) => map_row(record, session_id).ok(),
                None => None,
            };
            if let Some(scratch) = scratch {
                scratches.push(scratch);
            }
        }

        Ok(scratches)
    }

    /// Upsert a scratch record - creates if not exists, updates if exists.
    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        scratch_type: &ScratchType,
        data: &UpdateScratch,
    ) -> Result<Self, ScratchError> {
        let payload_value = serde_json::to_value(&data.payload)?;
        let scratch_type_str = scratch_type.to_string();
        let session_row_id = ids::session_id_by_uuid(db, id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let active = scratch::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            session_id: Set(session_row_id),
            scratch_type: Set(scratch_type_str.clone()),
            payload: Set(payload_value),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
            ..Default::default()
        };

        scratch::Entity::insert(active)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    scratch::Column::SessionId,
                    scratch::Column::ScratchType,
                ])
                .update_columns([scratch::Column::Payload, scratch::Column::UpdatedAt])
                .to_owned(),
            )
            .exec(db)
            .await?;

        let record = scratch::Entity::find()
            .filter(scratch::Column::SessionId.eq(session_row_id))
            .filter(scratch::Column::ScratchType.eq(scratch_type_str))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Scratch not found".to_string()))?;

        let payload = serde_json::to_value(ScratchEventPayload {
            scratch_id: id,
            scratch_type: scratch_type.to_string(),
        })
        .map_err(ScratchError::Serde)?;
        EventOutbox::enqueue(db, EVENT_SCRATCH_UPDATED, "scratch", id, payload).await?;
        map_row(record, id)
    }

    pub async fn delete<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        scratch_type: &ScratchType,
    ) -> Result<u64, DbErr> {
        let scratch_type_str = scratch_type.to_string();
        let session_row_id = ids::session_id_by_uuid(db, id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let result = scratch::Entity::delete_many()
            .filter(scratch::Column::SessionId.eq(session_row_id))
            .filter(scratch::Column::ScratchType.eq(scratch_type_str))
            .exec(db)
            .await?;
        if result.rows_affected > 0 {
            let payload = serde_json::to_value(ScratchEventPayload {
                scratch_id: id,
                scratch_type: scratch_type.to_string(),
            })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(db, EVENT_SCRATCH_DELETED, "scratch", id, payload).await?;
        }
        Ok(result.rows_affected)
    }
}
