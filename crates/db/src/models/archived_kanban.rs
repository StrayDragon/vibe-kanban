use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{archived_kanban, task},
    models::ids,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ArchivedKanban {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ArchivedKanbanWithTaskCount {
    #[serde(flatten)]
    #[ts(flatten)]
    pub archived_kanban: ArchivedKanban,
    pub tasks_count: u64,
}

impl ArchivedKanban {
    fn from_model_with_project_uuid(project_id: Uuid, model: archived_kanban::Model) -> Self {
        Self {
            id: model.uuid,
            project_id,
            title: model.title,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    async fn from_model<C: ConnectionTrait>(
        db: &C,
        model: archived_kanban::Model,
    ) -> Result<Self, DbErr> {
        let project_uuid = ids::project_uuid_by_id(db, model.project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        Ok(Self::from_model_with_project_uuid(project_uuid, model))
    }

    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = archived_kanban::Entity::find()
            .filter(archived_kanban::Column::Uuid.eq(id))
            .one(db)
            .await?;
        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    pub async fn list_by_project<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let records = archived_kanban::Entity::find()
            .filter(archived_kanban::Column::ProjectId.eq(project_row_id))
            .order_by_desc(archived_kanban::Column::CreatedAt)
            .all(db)
            .await?;

        Ok(records
            .into_iter()
            .map(|model| Self::from_model_with_project_uuid(project_id, model))
            .collect())
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
        title: String,
    ) -> Result<Self, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let now = Utc::now();
        let active = archived_kanban::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            project_id: Set(project_row_id),
            title: Set(title),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        Ok(Self::from_model_with_project_uuid(project_id, model))
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, DbErr> {
        let result = archived_kanban::Entity::delete_many()
            .filter(archived_kanban::Column::Uuid.eq(id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    pub async fn row_id_by_uuid<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<i64>, DbErr> {
        ids::archived_kanban_id_by_uuid(db, id).await
    }

    pub async fn tasks_count<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, DbErr> {
        let archive_row_id =
            ids::archived_kanban_id_by_uuid(db, id)
                .await?
                .ok_or(DbErr::RecordNotFound(
                    "Archived kanban not found".to_string(),
                ))?;
        task::Entity::find()
            .filter(task::Column::ArchivedKanbanId.eq(archive_row_id))
            .count(db)
            .await
    }

    pub async fn list_by_project_with_task_counts<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<ArchivedKanbanWithTaskCount>, DbErr> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let records = archived_kanban::Entity::find()
            .filter(archived_kanban::Column::ProjectId.eq(project_row_id))
            .order_by_desc(archived_kanban::Column::CreatedAt)
            .all(db)
            .await?;

        let mut out = Vec::with_capacity(records.len());
        for record in records {
            let tasks_count = task::Entity::find()
                .filter(task::Column::ArchivedKanbanId.eq(record.id))
                .count(db)
                .await?;

            out.push(ArchivedKanbanWithTaskCount {
                archived_kanban: Self::from_model_with_project_uuid(project_id, record),
                tasks_count,
            });
        }
        Ok(out)
    }
}
