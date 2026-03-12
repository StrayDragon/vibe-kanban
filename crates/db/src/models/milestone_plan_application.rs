use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{milestone, milestone_plan_application},
    models::ids,
    types::TaskCreatedByKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestonePlanApplicationSummary {
    pub id: Uuid,
    pub milestone_id: Uuid,
    pub schema_version: i32,
    pub applied_by_kind: TaskCreatedByKind,
    pub idempotency_key: Option<String>,
    pub applied_at: DateTime<Utc>,
}

impl MilestonePlanApplicationSummary {
    fn from_model(model: milestone_plan_application::Model, milestone_id: Uuid) -> Self {
        Self {
            id: model.uuid,
            milestone_id,
            schema_version: model.schema_version,
            applied_by_kind: model.applied_by_kind,
            idempotency_key: model.idempotency_key,
            applied_at: model.created_at.into(),
        }
    }
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    milestone_id: Uuid,
    schema_version: i32,
    plan_json: String,
    applied_by_kind: TaskCreatedByKind,
    idempotency_key: Option<String>,
    application_id: Uuid,
) -> Result<MilestonePlanApplicationSummary, DbErr> {
    let milestone_row_id = ids::milestone_id_by_uuid(db, milestone_id)
        .await?
        .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))?;

    let now = Utc::now();
    let active = milestone_plan_application::ActiveModel {
        uuid: Set(application_id),
        milestone_id: Set(milestone_row_id),
        schema_version: Set(schema_version),
        plan_json: Set(plan_json),
        applied_by_kind: Set(applied_by_kind),
        idempotency_key: Set(idempotency_key),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        ..Default::default()
    };

    let model = active.insert(db).await?;
    Ok(MilestonePlanApplicationSummary::from_model(
        model,
        milestone_id,
    ))
}

pub async fn find_latest_by_milestone_id<C: ConnectionTrait>(
    db: &C,
    milestone_id: Uuid,
) -> Result<Option<MilestonePlanApplicationSummary>, DbErr> {
    let milestone_row_id = ids::milestone_id_by_uuid(db, milestone_id)
        .await?
        .ok_or(DbErr::RecordNotFound("Milestone not found".to_string()))?;

    let record = milestone_plan_application::Entity::find()
        .filter(milestone_plan_application::Column::MilestoneId.eq(milestone_row_id))
        .order_by_desc(milestone_plan_application::Column::CreatedAt)
        .order_by_desc(milestone_plan_application::Column::Id)
        .one(db)
        .await?;

    Ok(record.map(|model| MilestonePlanApplicationSummary::from_model(model, milestone_id)))
}

pub async fn find_latest_by_milestone_row_id<C: ConnectionTrait>(
    db: &C,
    milestone_row_id: i64,
    milestone_id: Uuid,
) -> Result<Option<MilestonePlanApplicationSummary>, DbErr> {
    let record = milestone_plan_application::Entity::find()
        .filter(milestone_plan_application::Column::MilestoneId.eq(milestone_row_id))
        .order_by_desc(milestone_plan_application::Column::CreatedAt)
        .order_by_desc(milestone_plan_application::Column::Id)
        .one(db)
        .await?;

    Ok(record.map(|model| MilestonePlanApplicationSummary::from_model(model, milestone_id)))
}

pub async fn find_latest_by_milestone_row_ids<C: ConnectionTrait>(
    db: &C,
    milestone_row_ids: &[i64],
) -> Result<std::collections::HashMap<i64, MilestonePlanApplicationSummary>, DbErr> {
    use std::collections::HashMap;

    if milestone_row_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let milestone_id_map: HashMap<i64, Uuid> = milestone::Entity::find()
        .select_only()
        .column(milestone::Column::Id)
        .column(milestone::Column::Uuid)
        .filter(milestone::Column::Id.is_in(milestone_row_ids.to_vec()))
        .into_tuple::<(i64, Uuid)>()
        .all(db)
        .await?
        .into_iter()
        .collect();

    // Pull all records and pick latest per milestone id in-memory. This keeps the query
    // portable across sqlite and postgres without window functions.
    let records = milestone_plan_application::Entity::find()
        .filter(milestone_plan_application::Column::MilestoneId.is_in(milestone_row_ids.to_vec()))
        .order_by_desc(milestone_plan_application::Column::CreatedAt)
        .order_by_desc(milestone_plan_application::Column::Id)
        .all(db)
        .await?;

    let mut latest_by_row: HashMap<i64, MilestonePlanApplicationSummary> = HashMap::new();
    for model in records {
        if latest_by_row.contains_key(&model.milestone_id) {
            continue;
        }
        let milestone_row_id = model.milestone_id;
        let Some(milestone_uuid) = milestone_id_map.get(&model.milestone_id).copied() else {
            continue;
        };
        let summary = MilestonePlanApplicationSummary::from_model(model, milestone_uuid);
        latest_by_row.insert(milestone_row_id, summary);
    }

    Ok(latest_by_row)
}
