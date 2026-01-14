use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use utils::log_entries::LogEntryChannel;
use uuid::Uuid;

use crate::{
    entities::execution_process_log_entry,
    models::ids,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProcessLogEntry {
    pub execution_id: Uuid,
    pub channel: String,
    pub entry_index: i64,
    pub entry_json: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryRow {
    pub entry_index: i64,
    pub entry_json: String,
}

#[derive(Debug, Clone)]
pub struct LogEntryStats {
    pub count: i64,
    pub min_index: i64,
    pub max_index: i64,
}

fn to_db_channel(channel: LogEntryChannel) -> execution_process_log_entry::LogChannel {
    match channel {
        LogEntryChannel::Raw => execution_process_log_entry::LogChannel::Raw,
        LogEntryChannel::Normalized => execution_process_log_entry::LogChannel::Normalized,
    }
}

impl ExecutionProcessLogEntry {
    pub async fn table_available<C: ConnectionTrait>(db: &C) -> bool {
        execution_process_log_entry::Entity::find()
            .limit(1)
            .one(db)
            .await
            .is_ok()
    }

    pub async fn stats<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        channel: LogEntryChannel,
    ) -> Result<Option<LogEntryStats>, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let channel_value = to_db_channel(channel);

        let count = execution_process_log_entry::Entity::find()
            .filter(execution_process_log_entry::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_log_entry::Column::Channel.eq(channel_value.clone()))
            .count(db)
            .await?;

        if count == 0 {
            return Ok(None);
        }

        let min_row = execution_process_log_entry::Entity::find()
            .filter(execution_process_log_entry::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_log_entry::Column::Channel.eq(channel_value.clone()))
            .order_by_asc(execution_process_log_entry::Column::EntryIndex)
            .one(db)
            .await?;
        let max_row = execution_process_log_entry::Entity::find()
            .filter(execution_process_log_entry::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_log_entry::Column::Channel.eq(channel_value))
            .order_by_desc(execution_process_log_entry::Column::EntryIndex)
            .one(db)
            .await?;

        let min_index = min_row.map(|row| row.entry_index).unwrap_or(0);
        let max_index = max_row.map(|row| row.entry_index).unwrap_or(min_index);

        Ok(Some(LogEntryStats {
            count: i64::try_from(count).unwrap_or(i64::MAX),
            min_index,
            max_index,
        }))
    }

    pub async fn has_any<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        channel: LogEntryChannel,
    ) -> Result<bool, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let channel_value = to_db_channel(channel);

        let exists = execution_process_log_entry::Entity::find()
            .filter(execution_process_log_entry::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_log_entry::Column::Channel.eq(channel_value))
            .one(db)
            .await?
            .is_some();

        Ok(exists)
    }

    pub async fn fetch_page<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        channel: LogEntryChannel,
        limit: usize,
        cursor: Option<i64>,
    ) -> Result<Vec<LogEntryRow>, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let channel_value = to_db_channel(channel);
        let limit = i64::try_from(limit).unwrap_or(i64::MAX) as u64;

        let mut query = execution_process_log_entry::Entity::find()
            .filter(execution_process_log_entry::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_log_entry::Column::Channel.eq(channel_value));

        if let Some(cursor) = cursor {
            query = query.filter(execution_process_log_entry::Column::EntryIndex.lt(cursor));
        }

        let rows = query
            .order_by_desc(execution_process_log_entry::Column::EntryIndex)
            .limit(limit)
            .all(db)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| LogEntryRow {
                entry_index: row.entry_index,
                entry_json: row.entry_json.to_string(),
            })
            .collect())
    }

    pub async fn has_older<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        channel: LogEntryChannel,
        before_index: i64,
    ) -> Result<bool, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let channel_value = to_db_channel(channel);

        let exists = execution_process_log_entry::Entity::find()
            .filter(execution_process_log_entry::Column::ExecutionProcessId.eq(execution_row_id))
            .filter(execution_process_log_entry::Column::Channel.eq(channel_value))
            .filter(execution_process_log_entry::Column::EntryIndex.lt(before_index))
            .one(db)
            .await?
            .is_some();

        Ok(exists)
    }

    pub async fn upsert_entry<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        channel: LogEntryChannel,
        entry_index: i64,
        entry_json: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let channel_value = to_db_channel(channel);
        let json_value: serde_json::Value = serde_json::from_str(entry_json)
            .map_err(|err| DbErr::Custom(err.to_string()))?;

        let active = execution_process_log_entry::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            execution_process_id: Set(execution_row_id),
            channel: Set(channel_value),
            entry_index: Set(entry_index),
            entry_json: Set(json_value),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
            ..Default::default()
        };

        execution_process_log_entry::Entity::insert(active)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    execution_process_log_entry::Column::ExecutionProcessId,
                    execution_process_log_entry::Column::Channel,
                    execution_process_log_entry::Column::EntryIndex,
                ])
                .update_columns([
                    execution_process_log_entry::Column::EntryJson,
                    execution_process_log_entry::Column::UpdatedAt,
                ])
                .to_owned(),
            )
            .exec(db)
            .await?;

        Ok(())
    }

    pub async fn upsert_entries<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        channel: LogEntryChannel,
        entries: &[LogEntryRow],
    ) -> Result<(), DbErr> {
        if entries.is_empty() {
            return Ok(());
        }

        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let channel_value = to_db_channel(channel);

        let mut inserts = Vec::with_capacity(entries.len());
        for entry in entries {
            let json_value: serde_json::Value = serde_json::from_str(&entry.entry_json)
                .map_err(|err| DbErr::Custom(err.to_string()))?;
            inserts.push(execution_process_log_entry::ActiveModel {
                uuid: Set(Uuid::new_v4()),
                execution_process_id: Set(execution_row_id),
                channel: Set(channel_value.clone()),
                entry_index: Set(entry.entry_index),
                entry_json: Set(json_value),
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
                ..Default::default()
            });
        }

        execution_process_log_entry::Entity::insert_many(inserts)
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    execution_process_log_entry::Column::ExecutionProcessId,
                    execution_process_log_entry::Column::Channel,
                    execution_process_log_entry::Column::EntryIndex,
                ])
                .update_columns([
                    execution_process_log_entry::Column::EntryJson,
                    execution_process_log_entry::Column::UpdatedAt,
                ])
                .to_owned(),
            )
            .exec(db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    use super::*;

    #[tokio::test]
    async fn table_available_detects_missing_schema() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        assert!(!ExecutionProcessLogEntry::table_available(&db).await);

        db_migration::Migrator::up(&db, None).await.unwrap();
        assert!(ExecutionProcessLogEntry::table_available(&db).await);
    }
}
