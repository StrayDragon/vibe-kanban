use chrono::{DateTime, Duration as ChronoDuration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

use crate::entities::idempotency_key;

pub const IDEMPOTENCY_STATE_IN_PROGRESS: &str = "in_progress";
pub const IDEMPOTENCY_STATE_COMPLETED: &str = "completed";

#[derive(Debug, Clone)]
pub struct IdempotencyKey {
    pub uuid: Uuid,
    pub scope: String,
    pub key: String,
    pub request_hash: String,
    pub state: String,
    pub response_status: Option<i32>,
    pub response_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl IdempotencyKey {
    fn from_model(model: idempotency_key::Model) -> Self {
        Self {
            uuid: model.uuid,
            scope: model.scope,
            key: model.key,
            request_hash: model.request_hash,
            state: model.state,
            response_status: model.response_status,
            response_json: model.response_json,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub enum IdempotencyBeginOutcome {
    New { record_uuid: Uuid },
    Existing { record: IdempotencyKey },
}

pub async fn find_by_scope_key<C: ConnectionTrait>(
    db: &C,
    scope: &str,
    key: &str,
) -> Result<Option<IdempotencyKey>, DbErr> {
    let record = idempotency_key::Entity::find()
        .filter(idempotency_key::Column::Scope.eq(scope))
        .filter(idempotency_key::Column::Key.eq(key))
        .one(db)
        .await?;
    Ok(record.map(IdempotencyKey::from_model))
}

pub async fn begin<C: ConnectionTrait>(
    db: &C,
    scope: &str,
    key: &str,
    request_hash: &str,
    stale_in_progress_after: Option<ChronoDuration>,
) -> Result<IdempotencyBeginOutcome, DbErr> {
    if let Some(existing) = find_by_scope_key(db, scope, key).await? {
        if existing.state == IDEMPOTENCY_STATE_IN_PROGRESS
            && let Some(stale_after) = stale_in_progress_after
            && !stale_after.is_zero()
        {
            let age = Utc::now() - existing.created_at;
            if age > stale_after {
                tracing::warn!(
                    scope,
                    key,
                    record_uuid = %existing.uuid,
                    age_secs = age.num_seconds(),
                    stale_after_secs = stale_after.num_seconds(),
                    "Stale idempotency key found in_progress; deleting"
                );
                // Best-effort cleanup; if this fails, still return the existing record so callers
                // can decide what to do.
                if delete(db, existing.uuid).await.is_ok() {
                    // Record removed; treat this as a new request.
                } else {
                    return Ok(IdempotencyBeginOutcome::Existing { record: existing });
                }
            } else {
                return Ok(IdempotencyBeginOutcome::Existing { record: existing });
            }
        } else {
            return Ok(IdempotencyBeginOutcome::Existing { record: existing });
        }
    }

    let now = Utc::now();
    let active = idempotency_key::ActiveModel {
        uuid: Set(Uuid::new_v4()),
        scope: Set(scope.to_string()),
        key: Set(key.to_string()),
        request_hash: Set(request_hash.to_string()),
        state: Set(IDEMPOTENCY_STATE_IN_PROGRESS.to_string()),
        response_status: Set(None),
        response_json: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    };

    match active.insert(db).await {
        Ok(model) => Ok(IdempotencyBeginOutcome::New {
            record_uuid: model.uuid,
        }),
        Err(err) => {
            // Likely a concurrent insert; try to load the record and return it.
            if let Some(existing) = find_by_scope_key(db, scope, key).await? {
                return Ok(IdempotencyBeginOutcome::Existing { record: existing });
            }
            Err(err)
        }
    }
}

pub async fn complete<C: ConnectionTrait>(
    db: &C,
    record_uuid: Uuid,
    response_status: i32,
    response_json: String,
) -> Result<(), DbErr> {
    let record = idempotency_key::Entity::find()
        .filter(idempotency_key::Column::Uuid.eq(record_uuid))
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "Idempotency key not found".to_string(),
        ))?;

    let now = Utc::now();
    let mut active: idempotency_key::ActiveModel = record.into();
    active.state = Set(IDEMPOTENCY_STATE_COMPLETED.to_string());
    active.response_status = Set(Some(response_status));
    active.response_json = Set(Some(response_json));
    active.updated_at = Set(now.into());
    active.update(db).await?;
    Ok(())
}

pub async fn delete<C: ConnectionTrait>(db: &C, record_uuid: Uuid) -> Result<(), DbErr> {
    idempotency_key::Entity::delete_many()
        .filter(idempotency_key::Column::Uuid.eq(record_uuid))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn prune_completed_before<C: ConnectionTrait>(
    db: &C,
    cutoff: DateTime<Utc>,
) -> Result<u64, DbErr> {
    let result = idempotency_key::Entity::delete_many()
        .filter(idempotency_key::Column::State.eq(IDEMPOTENCY_STATE_COMPLETED))
        .filter(idempotency_key::Column::CreatedAt.lt(cutoff))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

pub async fn prune_in_progress_before<C: ConnectionTrait>(
    db: &C,
    cutoff: DateTime<Utc>,
) -> Result<u64, DbErr> {
    let result = idempotency_key::Entity::delete_many()
        .filter(idempotency_key::Column::State.eq(IDEMPOTENCY_STATE_IN_PROGRESS))
        .filter(idempotency_key::Column::CreatedAt.lt(cutoff))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
