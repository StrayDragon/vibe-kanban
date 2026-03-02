use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IntoActiveModel,
    QueryFilter, Set,
};
use uuid::Uuid;

use crate::entities::attempt_control_lease;

#[derive(Debug, Clone)]
pub struct AttemptControlLease {
    pub db_id: i64,
    pub id: String,
    pub attempt_id: Uuid,
    pub control_token: Uuid,
    pub claimed_by_client_id: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AttemptControlLease {
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }

    fn from_model(model: attempt_control_lease::Model) -> Self {
        Self {
            db_id: model.id,
            id: model.uuid.to_string(),
            attempt_id: model.attempt_id,
            control_token: model.control_token,
            claimed_by_client_id: model.claimed_by_client_id,
            expires_at: model.expires_at.into(),
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClaimOutcome {
    Claimed {
        lease: AttemptControlLease,
        token_rotated: bool,
    },
    Conflict {
        current: AttemptControlLease,
    },
}

pub async fn get_by_attempt_id<C: ConnectionTrait>(
    db: &C,
    attempt_id: Uuid,
) -> Result<Option<AttemptControlLease>, DbErr> {
    let record = attempt_control_lease::Entity::find()
        .filter(attempt_control_lease::Column::AttemptId.eq(attempt_id))
        .one(db)
        .await?;
    Ok(record.map(AttemptControlLease::from_model))
}

pub async fn claim<C: ConnectionTrait>(
    db: &C,
    attempt_id: Uuid,
    claimed_by_client_id: String,
    ttl: Duration,
    force: bool,
) -> Result<ClaimOutcome, DbErr> {
    let now = Utc::now();
    let expires_at = now + ttl;

    let record = attempt_control_lease::Entity::find()
        .filter(attempt_control_lease::Column::AttemptId.eq(attempt_id))
        .one(db)
        .await?;

    let Some(record) = record else {
        let active = attempt_control_lease::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            attempt_id: Set(attempt_id),
            control_token: Set(Uuid::new_v4()),
            claimed_by_client_id: Set(claimed_by_client_id),
            expires_at: Set(expires_at.into()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        let inserted = active.insert(db).await?;
        return Ok(ClaimOutcome::Claimed {
            lease: AttemptControlLease::from_model(inserted),
            token_rotated: true,
        });
    };

    let current = AttemptControlLease::from_model(record.clone());
    let expired = current.is_expired_at(now);
    let same_owner = current.claimed_by_client_id == claimed_by_client_id;

    if !force && !expired && !same_owner {
        return Ok(ClaimOutcome::Conflict { current });
    }

    let token_rotated = force || expired || !same_owner;
    let new_token = if token_rotated {
        Uuid::new_v4()
    } else {
        current.control_token
    };

    let mut active: attempt_control_lease::ActiveModel = record.into();
    active.control_token = Set(new_token);
    active.claimed_by_client_id = Set(claimed_by_client_id);
    active.expires_at = Set(expires_at.into());
    active.updated_at = Set(now.into());

    let updated = active.update(db).await?;
    Ok(ClaimOutcome::Claimed {
        lease: AttemptControlLease::from_model(updated),
        token_rotated,
    })
}

#[derive(Debug, Clone)]
pub enum ReleaseOutcome {
    Released,
    NotFound,
    TokenMismatch { current: AttemptControlLease },
}

pub async fn release<C: ConnectionTrait>(
    db: &C,
    attempt_id: Uuid,
    control_token: Uuid,
) -> Result<ReleaseOutcome, DbErr> {
    let record = attempt_control_lease::Entity::find()
        .filter(attempt_control_lease::Column::AttemptId.eq(attempt_id))
        .one(db)
        .await?;

    let Some(record) = record else {
        return Ok(ReleaseOutcome::NotFound);
    };

    let current = AttemptControlLease::from_model(record.clone());
    if current.control_token != control_token {
        return Ok(ReleaseOutcome::TokenMismatch { current });
    }

    attempt_control_lease::Entity::delete(record.into_active_model())
        .exec(db)
        .await?;

    Ok(ReleaseOutcome::Released)
}
