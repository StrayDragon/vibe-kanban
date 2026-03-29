use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
    sea_query::{Expr, ExprTrait},
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::event_outbox;

pub struct EventOutbox;

#[derive(Debug, Clone)]
pub struct EventOutboxEntry {
    pub id: i64,
    pub uuid: Uuid,
    pub event_type: String,
    pub entity_type: String,
    pub entity_uuid: Uuid,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

impl EventOutboxEntry {
    fn from_model(model: event_outbox::Model) -> Self {
        Self {
            id: model.id,
            uuid: model.uuid,
            event_type: model.event_type,
            entity_type: model.entity_type,
            entity_uuid: model.entity_uuid,
            payload: model.payload,
            created_at: model.created_at.into(),
            published_at: model.published_at.map(Into::into),
        }
    }
}

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
            .order_by_asc(event_outbox::Column::Id)
            .limit(limit)
            .all(db)
            .await
    }

    pub async fn mark_published<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), DbErr> {
        let result = event_outbox::Entity::update_many()
            .col_expr(
                event_outbox::Column::PublishedAt,
                Expr::value(Some::<sea_orm::prelude::DateTimeUtc>(Utc::now().into())),
            )
            .filter(event_outbox::Column::Id.eq(id))
            .exec(db)
            .await?;

        if result.rows_affected == 0 {
            return Err(DbErr::RecordNotFound(
                "Event outbox record not found".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn mark_failed<C: ConnectionTrait>(
        db: &C,
        id: i64,
        error: &str,
    ) -> Result<(), DbErr> {
        let result = event_outbox::Entity::update_many()
            .col_expr(
                event_outbox::Column::Attempts,
                Expr::col(event_outbox::Column::Attempts).add(Expr::val(1)),
            )
            .col_expr(
                event_outbox::Column::LastError,
                Expr::value(Some(error.to_string())),
            )
            .filter(event_outbox::Column::Id.eq(id))
            .exec(db)
            .await?;

        if result.rows_affected == 0 {
            return Err(DbErr::RecordNotFound(
                "Event outbox record not found".to_string(),
            ));
        }
        Ok(())
    }

    pub async fn tail_after<C: ConnectionTrait>(
        db: &C,
        after_id: i64,
        limit: u64,
    ) -> Result<Vec<EventOutboxEntry>, DbErr> {
        let limit = limit.clamp(1, 200);
        let records = event_outbox::Entity::find()
            .filter(event_outbox::Column::Id.gt(after_id))
            .order_by_asc(event_outbox::Column::Id)
            .limit(limit)
            .all(db)
            .await?;
        Ok(records
            .into_iter()
            .map(EventOutboxEntry::from_model)
            .collect())
    }

    pub async fn page_older<C: ConnectionTrait>(
        db: &C,
        cursor: Option<i64>,
        limit: u64,
    ) -> Result<(Vec<EventOutboxEntry>, Option<i64>, bool), DbErr> {
        let limit = limit.clamp(1, 200);
        let mut query = event_outbox::Entity::find().order_by_desc(event_outbox::Column::Id);
        if let Some(cursor) = cursor {
            query = query.filter(event_outbox::Column::Id.lt(cursor));
        }

        let mut records = query.limit(limit).all(db).await?;
        // Oldest -> newest for clients
        records.reverse();

        let next_cursor = records.first().map(|r| r.id);
        let has_more = if let Some(oldest) = next_cursor {
            event_outbox::Entity::find()
                .filter(event_outbox::Column::Id.lt(oldest))
                .select_only()
                .column(event_outbox::Column::Id)
                .limit(1)
                .into_tuple::<i64>()
                .one(db)
                .await?
                .is_some()
        } else {
            false
        };

        Ok((
            records
                .into_iter()
                .map(EventOutboxEntry::from_model)
                .collect(),
            next_cursor,
            has_more,
        ))
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
