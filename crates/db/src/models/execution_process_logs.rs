use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::log_msg::LogMsg;
use uuid::Uuid;

use crate::{
    entities::{execution_process, execution_process_log, execution_process_log_entry},
    models::ids,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ExecutionProcessLogs {
    pub execution_id: Uuid,
    pub logs: String,
    pub byte_size: i64,
    pub inserted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ExecutionProcessLogSummary {
    pub execution_id: Uuid,
    pub total_bytes: i64,
}

impl ExecutionProcessLogs {
    fn from_model(model: execution_process_log::Model, execution_id: Uuid) -> Self {
        Self {
            execution_id,
            logs: model.logs,
            byte_size: model.byte_size,
            inserted_at: model.inserted_at.into(),
        }
    }

    pub async fn list_execution_ids_with_bytes<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<ExecutionProcessLogSummary>, DbErr> {
        let records = execution_process_log::Entity::find().all(db).await?;

        let mut totals: HashMap<i64, (i64, DateTime<Utc>)> = HashMap::new();
        for record in records {
            let inserted_at: DateTime<Utc> = record.inserted_at.into();
            let entry = totals
                .entry(record.execution_process_id)
                .or_insert((0, inserted_at));
            entry.0 += record.byte_size;
            if inserted_at < entry.1 {
                entry.1 = inserted_at;
            }
        }

        let mut summaries = Vec::with_capacity(totals.len());
        for (execution_id, (total_bytes, earliest)) in totals {
            if let Some(uuid) = ids::execution_process_uuid_by_id(db, execution_id).await? {
                summaries.push((earliest, ExecutionProcessLogSummary {
                    execution_id: uuid,
                    total_bytes,
                }));
            }
        }

        summaries.sort_by_key(|(earliest, _)| *earliest);
        Ok(summaries.into_iter().map(|(_, summary)| summary).collect())
    }

    /// Find logs by execution process ID
    pub async fn find_by_execution_id<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
    ) -> Result<Vec<Self>, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let records = execution_process_log::Entity::find()
            .filter(execution_process_log::Column::ExecutionProcessId.eq(execution_row_id))
            .order_by_asc(execution_process_log::Column::InsertedAt)
            .all(db)
            .await?;

        Ok(records
            .into_iter()
            .map(|model| Self::from_model(model, execution_id))
            .collect())
    }

    pub async fn has_any<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
    ) -> Result<bool, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        Ok(execution_process_log::Entity::find()
            .filter(execution_process_log::Column::ExecutionProcessId.eq(execution_row_id))
            .one(db)
            .await?
            .is_some())
    }

    /// Parse JSONL logs back into Vec<LogMsg>
    pub fn parse_logs(records: &[Self]) -> Result<Vec<LogMsg>, serde_json::Error> {
        let mut messages = Vec::new();
        for line in records.iter().flat_map(|record| record.logs.lines()) {
            if !line.trim().is_empty() {
                let msg: LogMsg = serde_json::from_str(line)?;
                messages.push(msg);
            }
        }
        Ok(messages)
    }

    /// Append a JSONL line to the logs for an execution process
    pub async fn append_log_line<C: ConnectionTrait>(
        db: &C,
        execution_id: Uuid,
        jsonl_line: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;
        let byte_size = jsonl_line.len() as i64;

        let active = execution_process_log::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            execution_process_id: Set(execution_row_id),
            logs: Set(jsonl_line.to_string()),
            byte_size: Set(byte_size),
            inserted_at: Set(Utc::now().into()),
            ..Default::default()
        };
        active.insert(db).await?;
        Ok(())
    }

    pub async fn delete_legacy_for_completed_before<C: ConnectionTrait>(
        db: &C,
        completed_before: DateTime<Utc>,
    ) -> Result<u64, DbErr> {
        let completed_process_ids: Vec<i64> = execution_process::Entity::find()
            .select_only()
            .column(execution_process::Column::Id)
            .filter(execution_process::Column::CompletedAt.is_not_null())
            .filter(execution_process::Column::CompletedAt.lt(completed_before))
            .into_tuple()
            .all(db)
            .await?;

        if completed_process_ids.is_empty() {
            return Ok(0);
        }

        let mut process_ids_with_entries: Vec<i64> = execution_process_log_entry::Entity::find()
            .select_only()
            .column(execution_process_log_entry::Column::ExecutionProcessId)
            .filter(
                execution_process_log_entry::Column::ExecutionProcessId
                    .is_in(completed_process_ids),
            )
            .into_tuple()
            .all(db)
            .await?;

        process_ids_with_entries.sort_unstable();
        process_ids_with_entries.dedup();
        if process_ids_with_entries.is_empty() {
            return Ok(0);
        }

        let result = execution_process_log::Entity::delete_many()
            .filter(
                execution_process_log::Column::ExecutionProcessId.is_in(process_ids_with_entries),
            )
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use sea_orm::ActiveModelTrait;

    use utils::log_entries::LogEntryChannel;

    use crate::models::{
        execution_process_log_entries::ExecutionProcessLogEntry,
        project::{CreateProject, Project},
        session::{CreateSession, Session},
        task::{CreateTask, Task},
        workspace::{CreateWorkspace, Workspace},
    };
    use crate::types::{ExecutionProcessRunReason, ExecutionProcessStatus};

    use super::*;

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn create_session_row_id(db: &sea_orm::DatabaseConnection) -> i64 {
        let project_id = Uuid::new_v4();
        Project::create(
            db,
            &CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            db,
            &CreateTask::from_title_description(project_id, "Test task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let workspace_id = Uuid::new_v4();
        Workspace::create(
            db,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            workspace_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            db,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            workspace_id,
        )
        .await
        .unwrap();

        crate::models::ids::session_id_by_uuid(db, session_id)
            .await
            .unwrap()
            .expect("session row id")
    }

    #[tokio::test]
    async fn legacy_jsonl_cleanup_requires_log_entries() {
        let db = setup_db().await;
        let session_row_id = create_session_row_id(&db).await;

        let now = Utc::now();
        let completed_at = now - chrono::Duration::days(30);

        let execution_with_entries = Uuid::new_v4();
        execution_process::ActiveModel {
            uuid: Set(execution_with_entries),
            session_id: Set(session_row_id),
            run_reason: Set(ExecutionProcessRunReason::CodingAgent),
            executor_action: Set(serde_json::json!({})),
            status: Set(ExecutionProcessStatus::Completed),
            exit_code: Set(Some(0)),
            dropped: Set(false),
            started_at: Set(completed_at.into()),
            completed_at: Set(Some(completed_at.into())),
            created_at: Set(completed_at.into()),
            updated_at: Set(completed_at.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        ExecutionProcessLogEntry::upsert_entry(
            &db,
            execution_with_entries,
            LogEntryChannel::Raw,
            0,
            r#"{"type":"STDOUT","content":"hello"}"#,
        )
        .await
        .unwrap();

        let msg = LogMsg::Stdout("hello".to_string());
        let json_line = serde_json::to_string(&msg).unwrap();
        ExecutionProcessLogs::append_log_line(&db, execution_with_entries, &format!("{json_line}\n"))
            .await
            .unwrap();

        let execution_without_entries = Uuid::new_v4();
        execution_process::ActiveModel {
            uuid: Set(execution_without_entries),
            session_id: Set(session_row_id),
            run_reason: Set(ExecutionProcessRunReason::CodingAgent),
            executor_action: Set(serde_json::json!({})),
            status: Set(ExecutionProcessStatus::Completed),
            exit_code: Set(Some(0)),
            dropped: Set(false),
            started_at: Set(completed_at.into()),
            completed_at: Set(Some(completed_at.into())),
            created_at: Set(completed_at.into()),
            updated_at: Set(completed_at.into()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();

        let json_line = serde_json::to_string(&LogMsg::Stdout("world".to_string())).unwrap();
        ExecutionProcessLogs::append_log_line(&db, execution_without_entries, &format!("{json_line}\n"))
            .await
            .unwrap();

        let cutoff = now - chrono::Duration::days(14);
        let deleted = ExecutionProcessLogs::delete_legacy_for_completed_before(&db, cutoff)
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        assert!(
            ExecutionProcessLogs::find_by_execution_id(&db, execution_with_entries)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            !ExecutionProcessLogs::find_by_execution_id(&db, execution_without_entries)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
