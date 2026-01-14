use std::{str::FromStr, sync::Arc, time::Duration};

use anyhow::anyhow;
use db::{
    DBService,
    events::{
        EVENT_EXECUTION_PROCESS_CREATED, EVENT_EXECUTION_PROCESS_DELETED,
        EVENT_EXECUTION_PROCESS_UPDATED, EVENT_PROJECT_CREATED, EVENT_PROJECT_DELETED,
        EVENT_PROJECT_UPDATED, EVENT_SCRATCH_CREATED, EVENT_SCRATCH_DELETED,
        EVENT_SCRATCH_UPDATED, EVENT_TASK_CREATED, EVENT_TASK_DELETED, EVENT_TASK_UPDATED,
        EVENT_WORKSPACE_CREATED, EVENT_WORKSPACE_DELETED, EVENT_WORKSPACE_UPDATED,
        ExecutionProcessEventPayload, ProjectEventPayload, ScratchEventPayload, TaskEventPayload,
        WorkspaceEventPayload,
    },
    models::{
        event_outbox::EventOutbox,
        execution_process::ExecutionProcess,
        project::Project,
        scratch::{Scratch, ScratchType},
        session::Session,
        task::Task,
        workspace::Workspace,
    },
};
use tokio::sync::RwLock;
use utils::msg_store::MsgStore;
use uuid::Uuid;

#[path = "events/patches.rs"]
pub mod patches;
#[path = "events/streams.rs"]
mod streams;
#[path = "events/types.rs"]
pub mod types;

pub use patches::{
    execution_process_patch, project_patch, scratch_patch, task_patch, workspace_patch,
};
pub use types::EventError;

const OUTBOX_POLL_INTERVAL: Duration = Duration::from_millis(250);
const OUTBOX_BATCH_LIMIT: u64 = 100;

#[derive(Clone)]
pub struct EventService {
    msg_store: Arc<MsgStore>,
    db: DBService,
    #[allow(dead_code)]
    entry_count: Arc<RwLock<usize>>,
}

enum PatchKind {
    Add,
    Replace,
    Remove,
}

impl EventService {
    pub fn new(db: DBService, msg_store: Arc<MsgStore>, entry_count: Arc<RwLock<usize>>) -> Self {
        let service = Self {
            msg_store,
            db,
            entry_count,
        };
        service.spawn_outbox_worker();
        service
    }

    fn spawn_outbox_worker(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            service.run_outbox_loop().await;
        });
    }

    async fn run_outbox_loop(&self) {
        loop {
            if let Err(err) = self.flush_pending().await {
                tracing::error!(error = %err, "event outbox flush failed");
            }
            tokio::time::sleep(OUTBOX_POLL_INTERVAL).await;
        }
    }

    async fn flush_pending(&self) -> Result<(), EventError> {
        let entries = EventOutbox::fetch_unpublished(&self.db.pool, OUTBOX_BATCH_LIMIT).await?;
        if entries.is_empty() {
            return Ok(());
        }

        for entry in entries {
            match self.dispatch_entry(&entry).await {
                Ok(()) => {
                    EventOutbox::mark_published(&self.db.pool, entry.id).await?;
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    tracing::warn!(event_id = entry.uuid.to_string(), error = %err_msg, "event dispatch failed");
                    EventOutbox::mark_failed(&self.db.pool, entry.id, &err_msg).await?;
                }
            }
        }

        Ok(())
    }

    async fn dispatch_entry(&self, entry: &db::entities::event_outbox::Model) -> Result<(), EventError> {
        match entry.event_type.as_str() {
            EVENT_TASK_CREATED => {
                let payload: TaskEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_task_patch(payload, PatchKind::Add).await?;
            }
            EVENT_TASK_UPDATED => {
                let payload: TaskEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_task_patch(payload, PatchKind::Replace).await?;
            }
            EVENT_TASK_DELETED => {
                let payload: TaskEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_task_patch(payload, PatchKind::Remove).await?;
            }
            EVENT_PROJECT_CREATED => {
                let payload: ProjectEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_project_patch(payload.project_id, PatchKind::Add).await?;
            }
            EVENT_PROJECT_UPDATED => {
                let payload: ProjectEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_project_patch(payload.project_id, PatchKind::Replace).await?;
            }
            EVENT_PROJECT_DELETED => {
                let payload: ProjectEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_project_patch(payload.project_id, PatchKind::Remove).await?;
            }
            EVENT_WORKSPACE_CREATED | EVENT_WORKSPACE_UPDATED | EVENT_WORKSPACE_DELETED => {
                let payload: WorkspaceEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_task_patch_for_workspace(payload.task_id).await?;
            }
            EVENT_EXECUTION_PROCESS_CREATED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone())?;
                self.emit_execution_process_patch(payload.process_id, PatchKind::Add)
                    .await?;
                self.push_task_update_for_session(payload.session_id).await?;
            }
            EVENT_EXECUTION_PROCESS_UPDATED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone())?;
                self.emit_execution_process_patch(payload.process_id, PatchKind::Replace)
                    .await?;
                self.push_task_update_for_session(payload.session_id).await?;
            }
            EVENT_EXECUTION_PROCESS_DELETED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone())?;
                self.emit_execution_process_patch(payload.process_id, PatchKind::Remove)
                    .await?;
                self.push_task_update_for_session(payload.session_id).await?;
            }
            EVENT_SCRATCH_CREATED => {
                let payload: ScratchEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_scratch_patch(&payload, PatchKind::Add).await?;
            }
            EVENT_SCRATCH_UPDATED => {
                let payload: ScratchEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_scratch_patch(&payload, PatchKind::Replace).await?;
            }
            EVENT_SCRATCH_DELETED => {
                let payload: ScratchEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_scratch_patch(&payload, PatchKind::Remove).await?;
            }
            _ => {
                tracing::debug!(event_type = entry.event_type.as_str(), "unknown event type");
            }
        }

        Ok(())
    }

    async fn emit_task_patch(
        &self,
        payload: TaskEventPayload,
        kind: PatchKind,
    ) -> Result<(), EventError> {
        if matches!(kind, PatchKind::Remove) {
            self.msg_store.push_patch(task_patch::remove(payload.task_id));
            return Ok(());
        }

        let tasks = Task::find_by_project_id_with_attempt_status(&self.db.pool, payload.project_id)
            .await?;

        let task = tasks.into_iter().find(|t| t.id == payload.task_id);
        match (task, kind) {
            (Some(task), PatchKind::Add) => {
                self.msg_store.push_patch(task_patch::add(&task));
            }
            (Some(task), PatchKind::Replace) => {
                self.msg_store.push_patch(task_patch::replace(&task));
            }
            _ => {}
        }

        Ok(())
    }

    async fn emit_project_patch(
        &self,
        project_id: Uuid,
        kind: PatchKind,
    ) -> Result<(), EventError> {
        let project = Project::find_by_id(&self.db.pool, project_id).await?;
        match (project, kind) {
            (Some(project), PatchKind::Add) => {
                self.msg_store.push_patch(project_patch::add(&project));
            }
            (Some(project), PatchKind::Replace) => {
                self.msg_store.push_patch(project_patch::replace(&project));
            }
            (None, PatchKind::Remove) => {
                self.msg_store.push_patch(project_patch::remove(project_id));
            }
            (Some(_), PatchKind::Remove) => {
                self.msg_store.push_patch(project_patch::remove(project_id));
            }
            (None, _) => {}
        }

        Ok(())
    }

    async fn emit_execution_process_patch(
        &self,
        process_id: Uuid,
        kind: PatchKind,
    ) -> Result<(), EventError> {
        if matches!(kind, PatchKind::Remove) {
            self.msg_store
                .push_patch(execution_process_patch::remove(process_id));
            return Ok(());
        }

        let process = ExecutionProcess::find_by_id(&self.db.pool, process_id).await?;
        if let Some(process) = process {
            let patch = match kind {
                PatchKind::Add => execution_process_patch::add(&process),
                PatchKind::Replace => execution_process_patch::replace(&process),
                PatchKind::Remove => execution_process_patch::remove(process_id),
            };
            self.msg_store.push_patch(patch);
        }

        Ok(())
    }

    async fn emit_scratch_patch(
        &self,
        payload: &ScratchEventPayload,
        kind: PatchKind,
    ) -> Result<(), EventError> {
        if matches!(kind, PatchKind::Remove) {
            self.msg_store
                .push_patch(scratch_patch::remove(payload.scratch_id, &payload.scratch_type));
            return Ok(());
        }

        let scratch_type = ScratchType::from_str(&payload.scratch_type)
            .map_err(|err| EventError::Other(anyhow!("invalid scratch type: {err}")))?;

        let scratch = Scratch::find_by_id(&self.db.pool, payload.scratch_id, &scratch_type).await?;
        if let Some(scratch) = scratch {
            let patch = match kind {
                PatchKind::Add => scratch_patch::add(&scratch),
                PatchKind::Replace => scratch_patch::replace(&scratch),
                PatchKind::Remove => scratch_patch::remove(payload.scratch_id, &payload.scratch_type),
            };
            self.msg_store.push_patch(patch);
        }

        Ok(())
    }

    async fn emit_task_patch_for_workspace(&self, task_id: Uuid) -> Result<(), EventError> {
        let task = Task::find_by_id(&self.db.pool, task_id).await?;
        let Some(task) = task else {
            return Ok(());
        };

        let payload = TaskEventPayload {
            task_id,
            project_id: task.project_id,
        };
        self.emit_task_patch(payload, PatchKind::Replace).await
    }

    async fn push_task_update_for_session(&self, session_id: Uuid) -> Result<(), EventError> {
        let Some(session) = Session::find_by_id(&self.db.pool, session_id).await? else {
            return Ok(());
        };
        let Some(workspace) = Workspace::find_by_id(&self.db.pool, session.workspace_id).await? else {
            return Ok(());
        };
        self.emit_task_patch_for_workspace(workspace.task_id).await
    }

    pub fn msg_store(&self) -> &Arc<MsgStore> {
        &self.msg_store
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use utils::log_msg::LogMsg;

    use super::*;

    async fn setup_db() -> DBService {
        let pool = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&pool, None).await.unwrap();
        DBService { pool }
    }

    #[tokio::test]
    async fn flush_pending_publishes_outbox_and_emits_patches() {
        let db = setup_db().await;

        let project_id = Uuid::new_v4();
        Project::create(
            &db.pool,
            &db::models::project::CreateProject {
                name: "Test project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &db.pool,
            &db::models::task::CreateTask::from_title_description(
                project_id,
                "Test task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        EventOutbox::enqueue(
            &db.pool,
            EVENT_PROJECT_CREATED,
            "project",
            Uuid::new_v4(),
            serde_json::Value::Null,
        )
        .await
        .unwrap();

        let msg_store = Arc::new(MsgStore::new());
        let service = EventService {
            msg_store: msg_store.clone(),
            db: db.clone(),
            entry_count: Arc::new(RwLock::new(0)),
        };

        let before_flush = EventOutbox::fetch_unpublished(&service.db.pool, 10)
            .await
            .unwrap();
        assert_eq!(before_flush.len(), 3);

        service.flush_pending().await.unwrap();

        let unpublished_after = EventOutbox::fetch_unpublished(&service.db.pool, 10)
            .await
            .unwrap();
        assert_eq!(unpublished_after.len(), 1);
        assert_eq!(unpublished_after[0].attempts, 1);
        assert!(unpublished_after[0].last_error.is_some());

        let patch_count = msg_store
            .get_history()
            .into_iter()
            .filter(|msg| matches!(msg, LogMsg::JsonPatch(_)))
            .count();
        assert!(patch_count >= 2);
    }
}
