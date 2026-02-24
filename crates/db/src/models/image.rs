use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{image, task_image},
    models::ids,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Image {
    pub id: Uuid,
    pub file_path: String,
    pub original_name: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateImage {
    pub file_path: String,
    pub original_name: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskImage {
    pub id: Uuid,
    pub task_id: Uuid,
    pub image_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateTaskImage {
    pub task_id: Uuid,
    pub image_id: Uuid,
}

impl Image {
    fn from_model(model: image::Model) -> Self {
        Self {
            id: model.uuid,
            file_path: model.file_path,
            original_name: model.original_name,
            mime_type: model.mime_type,
            size_bytes: model.size_bytes,
            hash: model.hash,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub async fn create<C: ConnectionTrait>(db: &C, data: &CreateImage) -> Result<Self, DbErr> {
        let now = Utc::now();
        let active = image::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            file_path: Set(data.file_path.clone()),
            original_name: Set(data.original_name.clone()),
            mime_type: Set(data.mime_type.clone()),
            size_bytes: Set(data.size_bytes),
            hash: Set(data.hash.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        let model = active.insert(db).await?;
        Ok(Self::from_model(model))
    }

    pub async fn find_by_hash<C: ConnectionTrait>(
        db: &C,
        hash: &str,
    ) -> Result<Option<Self>, DbErr> {
        let record = image::Entity::find()
            .filter(image::Column::Hash.eq(hash))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = image::Entity::find()
            .filter(image::Column::Uuid.eq(id))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn find_by_file_path<C: ConnectionTrait>(
        db: &C,
        file_path: &str,
    ) -> Result<Option<Self>, DbErr> {
        let record = image::Entity::find()
            .filter(image::Column::FilePath.eq(file_path))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn find_by_task_id<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let task_row_id = match ids::task_id_by_uuid(db, task_id).await? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };

        let links = task_image::Entity::find()
            .filter(task_image::Column::TaskId.eq(task_row_id))
            .order_by_asc(task_image::Column::CreatedAt)
            .all(db)
            .await?;

        let mut images = Vec::with_capacity(links.len());
        for link in links {
            if let Some(model) = image::Entity::find_by_id(link.image_id).one(db).await? {
                images.push(Self::from_model(model));
            }
        }
        Ok(images)
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<(), DbErr> {
        image::Entity::delete_many()
            .filter(image::Column::Uuid.eq(id))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn find_orphaned_images<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let linked_ids: Vec<i64> = task_image::Entity::find()
            .select_only()
            .column(task_image::Column::ImageId)
            .into_tuple()
            .all(db)
            .await?;

        let records = if linked_ids.is_empty() {
            image::Entity::find().all(db).await?
        } else {
            image::Entity::find()
                .filter(image::Column::Id.is_not_in(linked_ids))
                .all(db)
                .await?
        };

        Ok(records.into_iter().map(Self::from_model).collect())
    }
}

impl TaskImage {
    /// Associate multiple images with a task, skipping duplicates.
    pub async fn associate_many_dedup<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        image_ids: &[Uuid],
    ) -> Result<(), DbErr> {
        let task_row_id = ids::task_id_by_uuid(db, task_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let mut inserts = Vec::new();
        for &image_id in image_ids {
            let image_row_id = ids::image_id_by_uuid(db, image_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Image not found".to_string()))?;
            inserts.push(task_image::ActiveModel {
                uuid: Set(Uuid::new_v4()),
                task_id: Set(task_row_id),
                image_id: Set(image_row_id),
                created_at: Set(Utc::now().into()),
                ..Default::default()
            });
        }

        if !inserts.is_empty() {
            task_image::Entity::insert_many(inserts)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::columns([
                        task_image::Column::TaskId,
                        task_image::Column::ImageId,
                    ])
                    .do_nothing()
                    .to_owned(),
                )
                .exec(db)
                .await?;
        }

        Ok(())
    }

    pub async fn delete_by_task_id<C: ConnectionTrait>(db: &C, task_id: Uuid) -> Result<(), DbErr> {
        let task_row_id = match ids::task_id_by_uuid(db, task_id).await? {
            Some(id) => id,
            None => return Ok(()),
        };

        task_image::Entity::delete_many()
            .filter(task_image::Column::TaskId.eq(task_row_id))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Check if an image is associated with a specific task.
    pub async fn is_associated<C: ConnectionTrait>(
        db: &C,
        task_id: Uuid,
        image_id: Uuid,
    ) -> Result<bool, DbErr> {
        let task_row_id = match ids::task_id_by_uuid(db, task_id).await? {
            Some(id) => id,
            None => return Ok(false),
        };
        let image_row_id = match ids::image_id_by_uuid(db, image_id).await? {
            Some(id) => id,
            None => return Ok(false),
        };

        let exists = task_image::Entity::find()
            .filter(task_image::Column::TaskId.eq(task_row_id))
            .filter(task_image::Column::ImageId.eq(image_row_id))
            .one(db)
            .await?
            .is_some();

        Ok(exists)
    }
}
