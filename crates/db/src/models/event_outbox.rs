use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::event_outbox;

pub struct EventOutbox;

impl EventOutbox {
    pub async fn enqueue<C: ConnectionTrait>(
        db: &C,
        event_type: &str,
        entity_type: &str,
        entity_uuid: Uuid,
        payload: Value,
    ) -> Result<(), DbErr> {
        let active = event_outbox::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            event_type: Set(event_type.to_string()),
            entity_type: Set(entity_type.to_string()),
            entity_uuid: Set(entity_uuid),
            payload: Set(payload),
            created_at: Set(Utc::now().into()),
            published_at: Set(None),
            attempts: Set(0),
            last_error: Set(None),
            ..Default::default()
        };

        active.insert(db).await?;
        Ok(())
    }

    pub async fn fetch_unpublished<C: ConnectionTrait>(
        db: &C,
        limit: u64,
    ) -> Result<Vec<event_outbox::Model>, DbErr> {
        event_outbox::Entity::find()
            .filter(event_outbox::Column::PublishedAt.is_null())
            .order_by_asc(event_outbox::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
    }

    pub async fn mark_published<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), DbErr> {
        let record =
            event_outbox::Entity::find_by_id(id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound(
                    "Event outbox record not found".to_string(),
                ))?;

        let mut active: event_outbox::ActiveModel = record.into();
        active.published_at = Set(Some(Utc::now().into()));
        active.update(db).await?;
        Ok(())
    }

    pub async fn mark_failed<C: ConnectionTrait>(
        db: &C,
        id: i64,
        error: &str,
    ) -> Result<(), DbErr> {
        let record =
            event_outbox::Entity::find_by_id(id)
                .one(db)
                .await?
                .ok_or(DbErr::RecordNotFound(
                    "Event outbox record not found".to_string(),
                ))?;

        let attempts = record.attempts + 1;
        let mut active: event_outbox::ActiveModel = record.into();
        active.attempts = Set(attempts);
        active.last_error = Set(Some(error.to_string()));
        active.update(db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    use super::*;

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    #[tokio::test]
    async fn outbox_enqueue_fetch_and_marking() {
        let db = setup_db().await;

        let entity_uuid_one = Uuid::new_v4();
        EventOutbox::enqueue(
            &db,
            "test.event.one",
            "test",
            entity_uuid_one,
            serde_json::json!({ "value": 1 }),
        )
        .await
        .unwrap();

        let entity_uuid_two = Uuid::new_v4();
        EventOutbox::enqueue(
            &db,
            "test.event.two",
            "test",
            entity_uuid_two,
            serde_json::json!({ "value": 2 }),
        )
        .await
        .unwrap();

        let entries = EventOutbox::fetch_unpublished(&db, 10).await.unwrap();
        assert_eq!(entries.len(), 2);

        let entry_one_id = entries
            .iter()
            .find(|entry| entry.entity_uuid == entity_uuid_one)
            .map(|entry| entry.id)
            .expect("entry one id");

        let entry_two_id = entries
            .iter()
            .find(|entry| entry.entity_uuid == entity_uuid_two)
            .map(|entry| entry.id)
            .expect("entry two id");

        EventOutbox::mark_published(&db, entry_one_id)
            .await
            .unwrap();
        let entries = EventOutbox::fetch_unpublished(&db, 10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entity_uuid, entity_uuid_two);

        EventOutbox::mark_failed(&db, entry_two_id, "boom")
            .await
            .unwrap();
        let entries = EventOutbox::fetch_unpublished(&db, 10).await.unwrap();
        let failed = entries
            .iter()
            .find(|entry| entry.id == entry_two_id)
            .expect("failed entry");
        assert_eq!(failed.attempts, 1);
        assert_eq!(failed.last_error.as_deref(), Some("boom"));

        EventOutbox::mark_published(&db, entry_two_id)
            .await
            .unwrap();
        assert!(
            EventOutbox::fetch_unpublished(&db, 10)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
