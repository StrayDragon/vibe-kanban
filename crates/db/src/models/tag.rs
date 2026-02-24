use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::entities::tag;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Tag {
    pub id: Uuid,
    pub tag_name: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateTag {
    pub tag_name: String,
    pub content: String,
}

#[derive(Debug, Deserialize, TS)]
pub struct UpdateTag {
    pub tag_name: Option<String>,
    pub content: Option<String>,
}

impl Tag {
    fn from_model(model: tag::Model) -> Self {
        Self {
            id: model.uuid,
            tag_name: model.tag_name,
            content: model.content,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let records = tag::Entity::find()
            .order_by_asc(tag::Column::TagName)
            .all(db)
            .await?;
        Ok(records.into_iter().map(Self::from_model).collect())
    }

    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = tag::Entity::find()
            .filter(tag::Column::Uuid.eq(id))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn create<C: ConnectionTrait>(db: &C, data: &CreateTag) -> Result<Self, DbErr> {
        let now = Utc::now();
        let active = tag::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            tag_name: Set(data.tag_name.clone()),
            content: Set(data.content.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        let model = active.insert(db).await?;
        Ok(Self::from_model(model))
    }

    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        data: &UpdateTag,
    ) -> Result<Self, DbErr> {
        let record = tag::Entity::find()
            .filter(tag::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Tag not found".to_string()))?;

        let mut active: tag::ActiveModel = record.into();
        if let Some(tag_name) = data.tag_name.clone() {
            active.tag_name = Set(tag_name);
        }
        if let Some(content) = data.content.clone() {
            active.content = Set(content);
        }
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;
        Ok(Self::from_model(updated))
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, DbErr> {
        let result = tag::Entity::delete_many()
            .filter(tag::Column::Uuid.eq(id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}
