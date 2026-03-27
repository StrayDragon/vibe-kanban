use std::{str::FromStr, sync::Arc, time::Duration};

use anyhow::anyhow;
use db::{
    DBService,
    events::{
        EVENT_EXECUTION_PROCESS_CREATED, EVENT_EXECUTION_PROCESS_DELETED,
        EVENT_EXECUTION_PROCESS_UPDATED, EVENT_PROJECT_CREATED, EVENT_PROJECT_DELETED,
        EVENT_PROJECT_UPDATED, EVENT_SCRATCH_CREATED, EVENT_SCRATCH_DELETED, EVENT_SCRATCH_UPDATED,
        EVENT_TASK_CREATED, EVENT_TASK_DELETED, EVENT_TASK_UPDATED, EVENT_WORKSPACE_CREATED,
        EVENT_WORKSPACE_DELETED, EVENT_WORKSPACE_UPDATED, ExecutionProcessEventPayload,
        ProjectEventPayload, ScratchEventPayload, TaskEventPayload, WorkspaceEventPayload,
    },
    models::{
        event_outbox::EventOutbox,
        execution_process::{ExecutionProcess, ExecutionProcessPublic},
        project::Project,
        scratch::{Scratch, ScratchType},
        session::Session,
        task::Task,
        workspace::Workspace,
    },
};
use logs_store::MsgStore;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod patches;
mod streams;
pub mod types;

pub use patches::{
    execution_process_patch, project_patch, scratch_patch, task_patch, workspace_patch,
};
pub use types::EventError;

const OUTBOX_POLL_INTERVAL: Duration = Duration::from_millis(250);
const OUTBOX_BATCH_LIMIT: u64 = 100;
const DISABLE_BACKGROUND_TASKS_ENV: &str = "VIBE_DISABLE_BACKGROUND_TASKS";

fn background_tasks_disabled() -> bool {
    match std::env::var(DISABLE_BACKGROUND_TASKS_ENV) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        Err(_) => false,
    }
}

#[derive(Clone)]
pub struct EventService {
    msg_store: Arc<MsgStore>,
    db: DBService,
    #[allow(dead_code)]
    entry_count: Arc<RwLock<usize>>,
    shutdown_token: CancellationToken,
}

enum PatchKind {
    Add,
    Replace,
    Remove,
}

impl EventService {
    pub fn new(
        db: DBService,
        msg_store: Arc<MsgStore>,
        entry_count: Arc<RwLock<usize>>,
        shutdown_token: CancellationToken,
    ) -> Self {
        let service = Self {
            msg_store,
            db,
            entry_count,
            shutdown_token,
        };
        if !background_tasks_disabled() {
            service.spawn_outbox_worker();
        }
        service
    }

    fn spawn_outbox_worker(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            service.run_outbox_loop().await;
        });
    }

    async fn run_outbox_loop(&self) {
        let shutdown_token = self.shutdown_token.clone();

        loop {
            let result = tokio::select! {
                _ = shutdown_token.cancelled() => {
                    tracing::info!("Stopping event outbox worker");
                    break;
                }
                result = self.flush_pending() => result,
            };

            match result {
                Ok(0) => {
                    tokio::select! {
                        _ = shutdown_token.cancelled() => {
                            tracing::info!("Stopping event outbox worker");
                            break;
                        }
                        _ = tokio::time::sleep(OUTBOX_POLL_INTERVAL) => {}
                    }
                }
                Ok(_) => {
                    // Drain as fast as possible when backlog exists, but yield to avoid starving
                    // request handling on single-thread runtimes.
                    tokio::task::yield_now().await;
                }
                Err(err) => {
                    tracing::error!(error = %err, "event outbox flush failed");
                    tokio::select! {
                        _ = shutdown_token.cancelled() => {
                            tracing::info!("Stopping event outbox worker");
                            break;
                        }
                        _ = tokio::time::sleep(OUTBOX_POLL_INTERVAL) => {}
                    }
                }
            }
        }
    }

    async fn flush_pending(&self) -> Result<usize, EventError> {
        let entries = EventOutbox::fetch_unpublished(&self.db.pool, OUTBOX_BATCH_LIMIT).await?;
        let entry_count = entries.len();
        if entry_count == 0 {
            return Ok(0);
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

        Ok(entry_count)
    }

    async fn dispatch_entry(
        &self,
        entry: &db::entities::event_outbox::Model,
    ) -> Result<(), EventError> {
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
                self.emit_project_patch(payload.project_id, PatchKind::Add)
                    .await?;
            }
            EVENT_PROJECT_UPDATED => {
                let payload: ProjectEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_project_patch(payload.project_id, PatchKind::Replace)
                    .await?;
            }
            EVENT_PROJECT_DELETED => {
                let payload: ProjectEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_project_patch(payload.project_id, PatchKind::Remove)
                    .await?;
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
                self.push_task_update_for_session(payload.session_id)
                    .await?;
            }
            EVENT_EXECUTION_PROCESS_UPDATED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone())?;
                self.emit_execution_process_patch(payload.process_id, PatchKind::Replace)
                    .await?;
                self.push_task_update_for_session(payload.session_id)
                    .await?;
            }
            EVENT_EXECUTION_PROCESS_DELETED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone())?;
                self.emit_execution_process_patch(payload.process_id, PatchKind::Remove)
                    .await?;
                self.push_task_update_for_session(payload.session_id)
                    .await?;
            }
            EVENT_SCRATCH_CREATED => {
                let payload: ScratchEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_scratch_patch(&payload, PatchKind::Add).await?;
            }
            EVENT_SCRATCH_UPDATED => {
                let payload: ScratchEventPayload = serde_json::from_value(entry.payload.clone())?;
                self.emit_scratch_patch(&payload, PatchKind::Replace)
                    .await?;
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
            self.msg_store
                .push_patch(task_patch::remove(payload.task_id));
            return Ok(());
        }

        // Prefer the "active task list" view so automation diagnostics remain consistent,
        // but fall back to fetching by id so archived task updates still broadcast.
        let mut task =
            Task::find_by_project_id_with_attempt_status(&self.db.pool, payload.project_id)
                .await?
                .into_iter()
                .find(|t| t.id == payload.task_id);

        if task.is_none() {
            task = Task::find_by_id_with_attempt_status(&self.db.pool, payload.task_id).await?;
        }

        if let Some(task) = task {
            match kind {
                PatchKind::Add => {
                    self.msg_store.push_patch(task_patch::add(&task));
                }
                PatchKind::Replace => {
                    self.msg_store.push_patch(task_patch::replace(&task));
                }
                PatchKind::Remove => {}
            }
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
            let process = ExecutionProcessPublic::from_process(&process);
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
            self.msg_store.push_patch(scratch_patch::remove(
                payload.scratch_id,
                &payload.scratch_type,
            ));
            return Ok(());
        }

        let scratch_type = ScratchType::from_str(&payload.scratch_type)
            .map_err(|err| EventError::Other(anyhow!("invalid scratch type: {err}")))?;

        let scratch = Scratch::find_by_id(&self.db.pool, payload.scratch_id, &scratch_type).await?;
        if let Some(scratch) = scratch {
            let patch = match kind {
                PatchKind::Add => scratch_patch::add(&scratch),
                PatchKind::Replace => scratch_patch::replace(&scratch),
                PatchKind::Remove => {
                    scratch_patch::remove(payload.scratch_id, &payload.scratch_type)
                }
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
        let Some(workspace) = Workspace::find_by_id(&self.db.pool, session.workspace_id).await?
        else {
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
    use logs_protocol::LogMsg;
    use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter, Set};
    use sea_orm_migration::MigratorTrait;

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
            shutdown_token: CancellationToken::new(),
        };

        let before_flush = EventOutbox::fetch_unpublished(&service.db.pool, 10)
            .await
            .unwrap();
        assert_eq!(before_flush.len(), 3);

        let processed = service.flush_pending().await.unwrap();
        assert!(processed > 0);

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

    #[tokio::test]
    async fn task_update_for_archived_task_emits_patch() {
        let db = setup_db().await;

        let project_id = Uuid::new_v4();
        Project::create(
            &db.pool,
            &db::models::project::CreateProject {
                name: "Archive project".to_string(),
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
                "Archive me".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        // Flush initial create events so the next flush only covers the archived update.
        {
            let msg_store = Arc::new(MsgStore::new());
            let service = EventService {
                msg_store,
                db: db.clone(),
                entry_count: Arc::new(RwLock::new(0)),
                shutdown_token: CancellationToken::new(),
            };

            loop {
                if service.flush_pending().await.unwrap() == 0 {
                    break;
                }
            }

            assert!(
                EventOutbox::fetch_unpublished(&service.db.pool, 10)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }

        let archive = db::models::archived_kanban::ArchivedKanban::create(
            &db.pool,
            project_id,
            "Archived".to_string(),
        )
        .await
        .unwrap();
        let archive_row_id =
            db::models::archived_kanban::ArchivedKanban::row_id_by_uuid(&db.pool, archive.id)
                .await
                .unwrap()
                .expect("archive row id");

        // Mark the task as archived via the underlying entity to mimic the archive flow.
        let record = db::entities::task::Entity::find()
            .filter(db::entities::task::Column::Uuid.eq(task_id))
            .one(&db.pool)
            .await
            .unwrap()
            .expect("task record");
        let mut active: db::entities::task::ActiveModel = record.into();
        active.archived_kanban_id = Set(Some(archive_row_id));
        active.update(&db.pool).await.unwrap();

        let payload = serde_json::to_value(TaskEventPayload {
            task_id,
            project_id,
        })
        .unwrap();
        EventOutbox::enqueue(&db.pool, EVENT_TASK_UPDATED, "task", task_id, payload)
            .await
            .unwrap();

        let msg_store = Arc::new(MsgStore::new());
        let service = EventService {
            msg_store: msg_store.clone(),
            db: db.clone(),
            entry_count: Arc::new(RwLock::new(0)),
            shutdown_token: CancellationToken::new(),
        };

        let processed = service.flush_pending().await.unwrap();
        assert_eq!(processed, 1);

        assert!(
            EventOutbox::fetch_unpublished(&service.db.pool, 10)
                .await
                .unwrap()
                .is_empty()
        );

        let task_path = format!("/tasks/{task_id}");
        let archive_id = archive.id.to_string();
        let emitted = msg_store.get_history().into_iter().any(|msg| {
            let LogMsg::JsonPatch(patch) = msg else {
                return false;
            };
            let Some(op) = patch.0.first() else {
                return false;
            };
            if op.path() != task_path.as_str() {
                return false;
            }
            match op {
                json_patch::PatchOperation::Replace(op) => {
                    op.value.get("archived_kanban_id").and_then(|v| v.as_str())
                        == Some(archive_id.as_str())
                }
                _ => false,
            }
        });

        assert!(emitted, "expected task patch for archived task update");
    }
}
