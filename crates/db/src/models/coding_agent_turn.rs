use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{coding_agent_turn, execution_process},
    models::ids,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CodingAgentTurn {
    pub id: Uuid,
    pub execution_process_id: Uuid,
    pub agent_session_id: Option<String>,
    pub prompt: Option<String>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateCodingAgentTurn {
    pub execution_process_id: Uuid,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IndexedCodingAgentTurn {
    pub entry_index: i64,
    pub turn_id: Uuid,
    pub prompt: Option<String>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CodingAgentTurnPage {
    pub entries: Vec<IndexedCodingAgentTurn>,
    pub next_cursor: Option<i64>,
    pub has_more: bool,
}

impl CodingAgentTurn {
    fn from_model(model: coding_agent_turn::Model, execution_process_id: Uuid) -> Self {
        Self {
            id: model.uuid,
            execution_process_id,
            agent_session_id: model.agent_session_id,
            prompt: model.prompt,
            summary: model.summary,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    /// Find coding agent turn by execution process ID
    pub async fn find_by_execution_process_id<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let record = coding_agent_turn::Entity::find()
            .filter(coding_agent_turn::Column::ExecutionProcessId.eq(execution_row_id))
            .one(db)
            .await?;

        Ok(record.map(|model| Self::from_model(model, execution_process_id)))
    }

    pub async fn find_by_agent_session_id<C: ConnectionTrait>(
        db: &C,
        agent_session_id: &str,
    ) -> Result<Option<Self>, DbErr> {
        let record = coding_agent_turn::Entity::find()
            .filter(coding_agent_turn::Column::AgentSessionId.eq(agent_session_id))
            .order_by_desc(coding_agent_turn::Column::UpdatedAt)
            .one(db)
            .await?;

        if let Some(model) = record {
            let execution_process_id =
                ids::execution_process_uuid_by_id(db, model.execution_process_id)
                    .await?
                    .ok_or(DbErr::RecordNotFound(
                        "Execution process not found".to_string(),
                    ))?;
            return Ok(Some(Self::from_model(model, execution_process_id)));
        }
        Ok(None)
    }

    /// Create a new coding agent turn
    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateCodingAgentTurn,
        id: Uuid,
    ) -> Result<Self, DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, data.execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let now = Utc::now();
        let active = coding_agent_turn::ActiveModel {
            uuid: Set(id),
            execution_process_id: Set(execution_row_id),
            agent_session_id: Set(None),
            prompt: Set(data.prompt.clone()),
            summary: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        Ok(Self::from_model(model, data.execution_process_id))
    }

    /// Update coding agent turn with agent session ID
    pub async fn update_agent_session_id<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
        agent_session_id: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let record = coding_agent_turn::Entity::find()
            .filter(coding_agent_turn::Column::ExecutionProcessId.eq(execution_row_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Coding agent turn not found".to_string(),
            ))?;

        let mut active: coding_agent_turn::ActiveModel = record.into();
        active.agent_session_id = Set(Some(agent_session_id.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    /// Update coding agent turn summary
    pub async fn update_summary<C: ConnectionTrait>(
        db: &C,
        execution_process_id: Uuid,
        summary: &str,
    ) -> Result<(), DbErr> {
        let execution_row_id = ids::execution_process_id_by_uuid(db, execution_process_id)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Execution process not found".to_string(),
            ))?;

        let record = coding_agent_turn::Entity::find()
            .filter(coding_agent_turn::Column::ExecutionProcessId.eq(execution_row_id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Coding agent turn not found".to_string(),
            ))?;

        let mut active: coding_agent_turn::ActiveModel = record.into();
        active.summary = Set(Some(summary.to_string()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        Ok(())
    }

    /// Tail coding agent turns for a session, tail-first with cursor paging for older history.
    pub async fn tail_by_session_id<C: ConnectionTrait>(
        db: &C,
        session_id: Uuid,
        limit: usize,
        cursor: Option<i64>,
    ) -> Result<CodingAgentTurnPage, DbErr> {
        let limit = limit.max(1);
        let session_row_id = ids::session_id_by_uuid(db, session_id)
            .await?
            .ok_or(DbErr::RecordNotFound("Session not found".to_string()))?;

        let process_ids: Vec<i64> = execution_process::Entity::find()
            .select_only()
            .column(execution_process::Column::Id)
            .filter(execution_process::Column::SessionId.eq(session_row_id))
            .filter(execution_process::Column::Dropped.eq(false))
            .into_tuple()
            .all(db)
            .await?;

        if process_ids.is_empty() {
            return Ok(CodingAgentTurnPage {
                entries: Vec::new(),
                next_cursor: None,
                has_more: false,
            });
        }

        let mut query = coding_agent_turn::Entity::find()
            .filter(coding_agent_turn::Column::ExecutionProcessId.is_in(process_ids))
            .order_by_desc(coding_agent_turn::Column::Id)
            .limit((limit.saturating_add(1)) as u64);

        if let Some(cursor) = cursor {
            query = query.filter(coding_agent_turn::Column::Id.lt(cursor));
        }

        let mut turns = query.all(db).await?;
        let has_more = turns.len() > limit;
        if has_more {
            turns.pop();
        }
        turns.reverse();

        let entries = turns
            .into_iter()
            .map(|turn| IndexedCodingAgentTurn {
                entry_index: turn.id,
                turn_id: turn.uuid,
                prompt: turn.prompt,
                summary: turn.summary,
                created_at: turn.created_at.into(),
                updated_at: turn.updated_at.into(),
            })
            .collect::<Vec<_>>();

        let next_cursor = entries.first().map(|entry| entry.entry_index);

        Ok(CodingAgentTurnPage {
            entries,
            next_cursor,
            has_more,
        })
    }
}
