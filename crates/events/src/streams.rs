use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use db::{
    DbPool,
    models::{
        execution_process::{ExecutionProcess, ExecutionProcessPublic},
        project::Project,
        scratch::Scratch,
        session::Session,
        task::{Task, TaskWithAttemptStatus},
    },
};
use futures::StreamExt;
use json_patch::{PatchOperation, RemoveOperation};
use logs_protocol::LogMsg;
use logs_store::{SequencedHistoryMetadata, SequencedLogMsg};
use serde_json::json;
use tokio::sync::RwLock;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use uuid::Uuid;

use super::{EventService, patches::execution_process_patch, types::EventError};

fn can_resume_from(after_seq: u64, meta: SequencedHistoryMetadata) -> bool {
    match meta.min_seq {
        Some(min) => after_seq >= min.saturating_sub(1),
        None => after_seq == 0,
    }
}

fn snapshot_seq_from_meta(meta: SequencedHistoryMetadata) -> u64 {
    meta.max_seq.unwrap_or(0)
}

impl EventService {
    /// Stream raw task messages for a specific project with initial snapshot.
    pub async fn stream_tasks_raw(
        &self,
        project_id: Option<Uuid>,
        include_archived: bool,
        archived_kanban_id: Option<Uuid>,
        after_seq: Option<u64>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<SequencedLogMsg, std::io::Error>>,
        EventError,
    > {
        fn build_tasks_snapshot(tasks: Vec<TaskWithAttemptStatus>) -> LogMsg {
            let tasks_map: serde_json::Map<String, serde_json::Value> = tasks
                .into_iter()
                .filter_map(|task| {
                    let task_id = task.id;
                    match serde_json::to_value(task) {
                        Ok(value) => Some((task_id.to_string(), value)),
                        Err(err) => {
                            tracing::error!(
                                task_id = %task_id,
                                error = %err,
                                "failed to serialize task for tasks snapshot"
                            );
                            None
                        }
                    }
                })
                .collect();

            let patch = json!([
                {
                    "op": "replace",
                    "path": "/tasks",
                    "value": tasks_map
                }
            ]);

            match serde_json::from_value(patch) {
                Ok(patch) => LogMsg::JsonPatch(patch),
                Err(err) => {
                    tracing::error!(error = %err, "failed to build tasks snapshot patch");
                    LogMsg::JsonPatch(json_patch::Patch(vec![]))
                }
            }
        }

        fn filter_task_patch(
            patch: json_patch::Patch,
            project_id: Option<Uuid>,
            include_archived: bool,
            archived_kanban_id: Option<Uuid>,
        ) -> Option<LogMsg> {
            let matches_filter = |task: &TaskWithAttemptStatus| {
                let project_ok = project_id.is_none_or(|id| task.project_id == id);
                let archived_ok = match archived_kanban_id {
                    Some(want) => task.archived_kanban_id == Some(want),
                    None => include_archived || task.archived_kanban_id.is_none(),
                };
                project_ok && archived_ok
            };

            if let Some(patch_op) = patch.0.first()
                && patch_op.path().starts_with("/tasks/")
            {
                match patch_op {
                    json_patch::PatchOperation::Add(op) => {
                        if let Ok(task) =
                            serde_json::from_value::<TaskWithAttemptStatus>(op.value.clone())
                        {
                            if matches_filter(&task) {
                                return Some(LogMsg::JsonPatch(patch));
                            }

                            let remove_patch =
                                json_patch::Patch(vec![PatchOperation::Remove(RemoveOperation {
                                    path: op.path.clone(),
                                })]);
                            return Some(LogMsg::JsonPatch(remove_patch));
                        }
                    }
                    json_patch::PatchOperation::Replace(op) => {
                        if let Ok(task) =
                            serde_json::from_value::<TaskWithAttemptStatus>(op.value.clone())
                        {
                            if matches_filter(&task) {
                                return Some(LogMsg::JsonPatch(patch));
                            }

                            let remove_patch =
                                json_patch::Patch(vec![PatchOperation::Remove(RemoveOperation {
                                    path: op.path.clone(),
                                })]);
                            return Some(LogMsg::JsonPatch(remove_patch));
                        }
                    }
                    json_patch::PatchOperation::Remove(_) => {
                        return Some(LogMsg::JsonPatch(patch));
                    }
                    _ => {}
                }
            }

            None
        }

        let (history, receiver, meta) = self.msg_store.subscribe_sequenced_from(after_seq);
        let snapshot_seq = snapshot_seq_from_meta(meta);
        let needs_snapshot = after_seq.is_none() || !can_resume_from(after_seq.unwrap_or(0), meta);

        let mut initial_msgs: Vec<SequencedLogMsg> = Vec::new();
        let initial_last_seq: u64;

        if needs_snapshot {
            let tasks = Task::find_filtered_with_attempt_status(
                &self.db.pool,
                project_id,
                include_archived,
                archived_kanban_id,
            )
            .await?;
            initial_msgs.push(SequencedLogMsg {
                seq: snapshot_seq,
                msg: build_tasks_snapshot(tasks),
            });
            initial_last_seq = snapshot_seq;
        } else {
            for item in history {
                match item.msg {
                    LogMsg::JsonPatch(patch) => {
                        if let Some(msg) = filter_task_patch(
                            patch,
                            project_id,
                            include_archived,
                            archived_kanban_id,
                        ) {
                            initial_msgs.push(SequencedLogMsg { seq: item.seq, msg });
                        }
                    }
                    other => initial_msgs.push(SequencedLogMsg {
                        seq: item.seq,
                        msg: other,
                    }),
                }
            }
            initial_last_seq = initial_msgs
                .last()
                .map(|msg| msg.seq)
                .unwrap_or(after_seq.unwrap_or(0));
        }

        let db_pool = self.db.pool.clone();
        let msg_store = Arc::clone(&self.msg_store);
        let last_seq = Arc::new(AtomicU64::new(initial_last_seq));
        let project_filter = project_id;
        let archived_filter = archived_kanban_id;

        let filtered_stream = BroadcastStream::new(receiver).filter_map(move |msg_result| {
            let db_pool = db_pool.clone();
            let msg_store = Arc::clone(&msg_store);
            let last_seq = Arc::clone(&last_seq);
            async move {
                match msg_result {
                    Ok(item) => {
                        if item.seq <= last_seq.load(Ordering::Relaxed) {
                            return None;
                        }

                        let msg = match item.msg {
                            LogMsg::JsonPatch(patch) => filter_task_patch(
                                patch,
                                project_filter,
                                include_archived,
                                archived_filter,
                            )?,
                            other => other,
                        };

                        last_seq.store(item.seq, Ordering::Relaxed);
                        Some(Ok(SequencedLogMsg { seq: item.seq, msg }))
                    }
                    Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                        let watermark = msg_store.max_seq().unwrap_or(0);
                        tracing::warn!(
                            skipped = skipped,
                            watermark = watermark,
                            "tasks stream lagged; resyncing snapshot"
                        );

                        match Task::find_filtered_with_attempt_status(
                            &db_pool,
                            project_filter,
                            include_archived,
                            archived_filter,
                        )
                        .await
                        {
                            Ok(tasks) => {
                                last_seq.store(watermark, Ordering::Relaxed);
                                Some(Ok(SequencedLogMsg {
                                    seq: watermark,
                                    msg: build_tasks_snapshot(tasks),
                                }))
                            }
                            Err(err) => {
                                tracing::error!(
                                    error = %err,
                                    "failed to resync tasks after lag"
                                );
                                Some(Err(std::io::Error::other(format!(
                                    "failed to resync tasks after lag: {err}"
                                ))))
                            }
                        }
                    }
                }
            }
        });

        let initial_stream =
            futures::stream::iter(initial_msgs.into_iter().map(Ok::<_, std::io::Error>));
        Ok(initial_stream.chain(filtered_stream).boxed())
    }

    /// Stream raw project messages with initial snapshot.
    pub async fn stream_projects_raw(
        &self,
        after_seq: Option<u64>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<SequencedLogMsg, std::io::Error>>,
        EventError,
    > {
        fn build_projects_snapshot(projects: Vec<Project>) -> LogMsg {
            let projects_map: serde_json::Map<String, serde_json::Value> = projects
                .into_iter()
                .filter_map(|project| {
                    let project_id = project.id;
                    match serde_json::to_value(project) {
                        Ok(value) => Some((project_id.to_string(), value)),
                        Err(err) => {
                            tracing::error!(
                                project_id = %project_id,
                                error = %err,
                                "failed to serialize project for projects snapshot"
                            );
                            None
                        }
                    }
                })
                .collect();

            let patch = json!([
                {
                    "op": "replace",
                    "path": "/projects",
                    "value": projects_map
                }
            ]);

            match serde_json::from_value(patch) {
                Ok(patch) => LogMsg::JsonPatch(patch),
                Err(err) => {
                    tracing::error!(error = %err, "failed to build projects snapshot patch");
                    LogMsg::JsonPatch(json_patch::Patch(vec![]))
                }
            }
        }

        fn patch_is_projects(patch: &json_patch::Patch) -> bool {
            patch
                .0
                .first()
                .is_some_and(|op| op.path().starts_with("/projects"))
        }

        let (history, receiver, meta) = self.msg_store.subscribe_sequenced_from(after_seq);
        let snapshot_seq = snapshot_seq_from_meta(meta);
        let needs_snapshot = after_seq.is_none() || !can_resume_from(after_seq.unwrap_or(0), meta);

        let mut initial_msgs: Vec<SequencedLogMsg> = Vec::new();
        let initial_last_seq: u64;

        if needs_snapshot {
            let projects = Project::find_all(&self.db.pool).await?;
            initial_msgs.push(SequencedLogMsg {
                seq: snapshot_seq,
                msg: build_projects_snapshot(projects),
            });
            initial_last_seq = snapshot_seq;
        } else {
            for item in history {
                match item.msg {
                    LogMsg::JsonPatch(patch) => {
                        if patch_is_projects(&patch) {
                            initial_msgs.push(SequencedLogMsg {
                                seq: item.seq,
                                msg: LogMsg::JsonPatch(patch),
                            });
                        }
                    }
                    other => initial_msgs.push(SequencedLogMsg {
                        seq: item.seq,
                        msg: other,
                    }),
                }
            }
            initial_last_seq = initial_msgs
                .last()
                .map(|msg| msg.seq)
                .unwrap_or(after_seq.unwrap_or(0));
        }

        let db_pool = self.db.pool.clone();
        let msg_store = Arc::clone(&self.msg_store);
        let last_seq = Arc::new(AtomicU64::new(initial_last_seq));

        let filtered_stream = BroadcastStream::new(receiver).filter_map(move |msg_result| {
            let db_pool = db_pool.clone();
            let msg_store = Arc::clone(&msg_store);
            let last_seq = Arc::clone(&last_seq);
            async move {
                match msg_result {
                    Ok(item) => {
                        if item.seq <= last_seq.load(Ordering::Relaxed) {
                            return None;
                        }

                        match item.msg {
                            LogMsg::JsonPatch(patch) => {
                                if patch_is_projects(&patch) {
                                    last_seq.store(item.seq, Ordering::Relaxed);
                                    return Some(Ok(SequencedLogMsg {
                                        seq: item.seq,
                                        msg: LogMsg::JsonPatch(patch),
                                    }));
                                }
                                None
                            }
                            other => {
                                last_seq.store(item.seq, Ordering::Relaxed);
                                Some(Ok(SequencedLogMsg {
                                    seq: item.seq,
                                    msg: other,
                                }))
                            }
                        }
                    }
                    Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                        let watermark = msg_store.max_seq().unwrap_or(0);
                        tracing::warn!(
                            skipped = skipped,
                            watermark = watermark,
                            "projects stream lagged; resyncing snapshot"
                        );

                        match Project::find_all(&db_pool).await {
                            Ok(projects) => {
                                last_seq.store(watermark, Ordering::Relaxed);
                                Some(Ok(SequencedLogMsg {
                                    seq: watermark,
                                    msg: build_projects_snapshot(projects),
                                }))
                            }
                            Err(err) => {
                                tracing::error!(
                                    error = %err,
                                    "failed to resync projects after lag"
                                );
                                Some(Err(std::io::Error::other(format!(
                                    "failed to resync projects after lag: {err}"
                                ))))
                            }
                        }
                    }
                }
            }
        });

        let initial_stream =
            futures::stream::iter(initial_msgs.into_iter().map(Ok::<_, std::io::Error>));
        Ok(initial_stream.chain(filtered_stream).boxed())
    }

    /// Stream execution processes for a specific workspace with initial snapshot (raw LogMsg format for WebSocket).
    pub async fn stream_execution_processes_for_workspace_raw(
        &self,
        workspace_id: Uuid,
        show_soft_deleted: bool,
        after_seq: Option<u64>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<SequencedLogMsg, std::io::Error>>,
        EventError,
    > {
        async fn load_workspace_session_ids(
            db_pool: &DbPool,
            workspace_id: Uuid,
        ) -> Result<HashSet<Uuid>, EventError> {
            let sessions = Session::find_by_workspace_id(db_pool, workspace_id).await?;
            Ok(sessions.into_iter().map(|s| s.id).collect())
        }

        async fn build_execution_processes_snapshot(
            db_pool: &DbPool,
            workspace_id: Uuid,
            show_soft_deleted: bool,
        ) -> Result<(LogMsg, HashSet<Uuid>), EventError> {
            let sessions = Session::find_by_workspace_id(db_pool, workspace_id).await?;

            let mut all_processes = Vec::new();
            for session in &sessions {
                let processes =
                    ExecutionProcess::find_by_session_id(db_pool, session.id, show_soft_deleted)
                        .await?;
                all_processes.extend(processes);
            }

            let session_ids = sessions.iter().map(|s| s.id).collect::<HashSet<_>>();

            let processes_map: serde_json::Map<String, serde_json::Value> = all_processes
                .into_iter()
                .filter_map(|process| {
                    let process_id = process.id;
                    let public = ExecutionProcessPublic::from_process(&process);
                    match serde_json::to_value(public) {
                        Ok(value) => Some((process_id.to_string(), value)),
                        Err(err) => {
                            tracing::error!(
                                execution_process_id = %process_id,
                                error = %err,
                                "failed to serialize execution process for snapshot"
                            );
                            None
                        }
                    }
                })
                .collect();

            let initial_patch = json!([{
                "op": "replace",
                "path": "/execution_processes",
                "value": processes_map
            }]);
            let initial_msg = match serde_json::from_value(initial_patch) {
                Ok(patch) => LogMsg::JsonPatch(patch),
                Err(err) => {
                    tracing::error!(
                        error = %err,
                        "failed to build execution processes snapshot patch"
                    );
                    LogMsg::JsonPatch(json_patch::Patch(vec![]))
                }
            };

            Ok((initial_msg, session_ids))
        }

        async fn session_matches_workspace(
            session_ids: &RwLock<HashSet<Uuid>>,
            db_pool: &DbPool,
            workspace_id: Uuid,
            session_id: Uuid,
        ) -> bool {
            if session_ids.read().await.contains(&session_id) {
                return true;
            }

            match Session::find_by_id(db_pool, session_id).await {
                Ok(Some(session)) if session.workspace_id == workspace_id => {
                    session_ids.write().await.insert(session_id);
                    true
                }
                Ok(_) => false,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        session_id = %session_id,
                        "failed to validate session for execution process stream"
                    );
                    false
                }
            }
        }

        fn patch_is_execution_processes(patch: &json_patch::Patch) -> bool {
            patch
                .0
                .first()
                .is_some_and(|op| op.path().starts_with("/execution_processes/"))
        }

        let (history, receiver, meta) = self.msg_store.subscribe_sequenced_from(after_seq);
        let snapshot_seq = snapshot_seq_from_meta(meta);
        let needs_snapshot = after_seq.is_none() || !can_resume_from(after_seq.unwrap_or(0), meta);

        let mut initial_msgs: Vec<SequencedLogMsg> = Vec::new();
        let session_ids = if needs_snapshot {
            let (snapshot, session_ids) =
                build_execution_processes_snapshot(&self.db.pool, workspace_id, show_soft_deleted)
                    .await?;
            initial_msgs.push(SequencedLogMsg {
                seq: snapshot_seq,
                msg: snapshot,
            });
            session_ids
        } else {
            load_workspace_session_ids(&self.db.pool, workspace_id).await?
        };

        let session_ids = Arc::new(RwLock::new(session_ids));

        let initial_last_seq: u64 = if needs_snapshot {
            snapshot_seq
        } else {
            for item in history {
                match item.msg {
                    LogMsg::JsonPatch(patch) => {
                        if !patch_is_execution_processes(&patch) {
                            continue;
                        }

                        let Some(op) = patch.0.first() else {
                            continue;
                        };

                        match op {
                            json_patch::PatchOperation::Add(add) => {
                                if let Ok(process) = serde_json::from_value::<ExecutionProcessPublic>(
                                    add.value.clone(),
                                ) && session_matches_workspace(
                                    &session_ids,
                                    &self.db.pool,
                                    workspace_id,
                                    process.session_id,
                                )
                                .await
                                {
                                    if !show_soft_deleted && process.dropped {
                                        initial_msgs.push(SequencedLogMsg {
                                            seq: item.seq,
                                            msg: LogMsg::JsonPatch(
                                                execution_process_patch::remove(process.id),
                                            ),
                                        });
                                    } else {
                                        initial_msgs.push(SequencedLogMsg {
                                            seq: item.seq,
                                            msg: LogMsg::JsonPatch(patch),
                                        });
                                    }
                                }
                            }
                            json_patch::PatchOperation::Replace(replace) => {
                                if let Ok(process) = serde_json::from_value::<ExecutionProcessPublic>(
                                    replace.value.clone(),
                                ) && session_matches_workspace(
                                    &session_ids,
                                    &self.db.pool,
                                    workspace_id,
                                    process.session_id,
                                )
                                .await
                                {
                                    if !show_soft_deleted && process.dropped {
                                        initial_msgs.push(SequencedLogMsg {
                                            seq: item.seq,
                                            msg: LogMsg::JsonPatch(
                                                execution_process_patch::remove(process.id),
                                            ),
                                        });
                                    } else {
                                        initial_msgs.push(SequencedLogMsg {
                                            seq: item.seq,
                                            msg: LogMsg::JsonPatch(patch),
                                        });
                                    }
                                }
                            }
                            json_patch::PatchOperation::Remove(_) => {
                                initial_msgs.push(SequencedLogMsg {
                                    seq: item.seq,
                                    msg: LogMsg::JsonPatch(patch),
                                });
                            }
                            _ => {}
                        }
                    }
                    other => initial_msgs.push(SequencedLogMsg {
                        seq: item.seq,
                        msg: other,
                    }),
                }
            }

            initial_msgs
                .last()
                .map(|msg| msg.seq)
                .unwrap_or(after_seq.unwrap_or(0))
        };

        let db_pool = self.db.pool.clone();
        let msg_store = Arc::clone(&self.msg_store);
        let last_seq = Arc::new(AtomicU64::new(initial_last_seq));

        let filtered_stream = BroadcastStream::new(receiver).filter_map(move |msg_result| {
            let session_ids = Arc::clone(&session_ids);
            let db_pool = db_pool.clone();
            let msg_store = Arc::clone(&msg_store);
            let last_seq = Arc::clone(&last_seq);
            async move {
                match msg_result {
                    Ok(item) => {
                        if item.seq <= last_seq.load(Ordering::Relaxed) {
                            return None;
                        }

                        match item.msg {
                            LogMsg::JsonPatch(patch) => {
                                if !patch_is_execution_processes(&patch) {
                                    return None;
                                }

                                let op = patch.0.first()?;

                                match op {
                                    json_patch::PatchOperation::Add(add) => {
                                        if let Ok(process) =
                                            serde_json::from_value::<ExecutionProcessPublic>(
                                                add.value.clone(),
                                            )
                                            && session_matches_workspace(
                                                &session_ids,
                                                &db_pool,
                                                workspace_id,
                                                process.session_id,
                                            )
                                            .await
                                        {
                                            last_seq.store(item.seq, Ordering::Relaxed);
                                            if !show_soft_deleted && process.dropped {
                                                return Some(Ok(SequencedLogMsg {
                                                    seq: item.seq,
                                                    msg: LogMsg::JsonPatch(
                                                        execution_process_patch::remove(process.id),
                                                    ),
                                                }));
                                            }
                                            return Some(Ok(SequencedLogMsg {
                                                seq: item.seq,
                                                msg: LogMsg::JsonPatch(patch),
                                            }));
                                        }
                                    }
                                    json_patch::PatchOperation::Replace(replace) => {
                                        if let Ok(process) =
                                            serde_json::from_value::<ExecutionProcessPublic>(
                                                replace.value.clone(),
                                            )
                                            && session_matches_workspace(
                                                &session_ids,
                                                &db_pool,
                                                workspace_id,
                                                process.session_id,
                                            )
                                            .await
                                        {
                                            last_seq.store(item.seq, Ordering::Relaxed);
                                            if !show_soft_deleted && process.dropped {
                                                return Some(Ok(SequencedLogMsg {
                                                    seq: item.seq,
                                                    msg: LogMsg::JsonPatch(
                                                        execution_process_patch::remove(process.id),
                                                    ),
                                                }));
                                            }
                                            return Some(Ok(SequencedLogMsg {
                                                seq: item.seq,
                                                msg: LogMsg::JsonPatch(patch),
                                            }));
                                        }
                                    }
                                    json_patch::PatchOperation::Remove(_) => {
                                        last_seq.store(item.seq, Ordering::Relaxed);
                                        return Some(Ok(SequencedLogMsg {
                                            seq: item.seq,
                                            msg: LogMsg::JsonPatch(patch),
                                        }));
                                    }
                                    _ => {}
                                }

                                None
                            }
                            other => {
                                last_seq.store(item.seq, Ordering::Relaxed);
                                Some(Ok(SequencedLogMsg {
                                    seq: item.seq,
                                    msg: other,
                                }))
                            }
                        }
                    }
                    Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                        let watermark = msg_store.max_seq().unwrap_or(0);
                        tracing::warn!(
                            skipped = skipped,
                            watermark = watermark,
                            "execution process stream lagged; resyncing snapshot"
                        );

                        match build_execution_processes_snapshot(
                            &db_pool,
                            workspace_id,
                            show_soft_deleted,
                        )
                        .await
                        {
                            Ok((snapshot, refreshed_session_ids)) => {
                                *session_ids.write().await = refreshed_session_ids;
                                last_seq.store(watermark, Ordering::Relaxed);
                                Some(Ok(SequencedLogMsg {
                                    seq: watermark,
                                    msg: snapshot,
                                }))
                            }
                            Err(err) => {
                                tracing::error!(
                                    error = %err,
                                    "failed to resync execution processes after lag"
                                );
                                Some(Err(std::io::Error::other(format!(
                                    "failed to resync execution processes after lag: {err}"
                                ))))
                            }
                        }
                    }
                }
            }
        });

        let initial_stream =
            futures::stream::iter(initial_msgs.into_iter().map(Ok::<_, std::io::Error>));
        Ok(initial_stream.chain(filtered_stream).boxed())
    }

    /// Stream a single scratch item with initial snapshot (raw LogMsg format for WebSocket).
    pub async fn stream_scratch_raw(
        &self,
        scratch_id: Uuid,
        scratch_type: &db::models::scratch::ScratchType,
        after_seq: Option<u64>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<SequencedLogMsg, std::io::Error>>,
        EventError,
    > {
        fn build_scratch_snapshot(scratch: Option<Scratch>) -> LogMsg {
            let patch = json!([{
                "op": "replace",
                "path": "/scratch",
                "value": scratch
            }]);

            match serde_json::from_value(patch) {
                Ok(patch) => LogMsg::JsonPatch(patch),
                Err(err) => {
                    tracing::error!(error = %err, "failed to build scratch snapshot patch");
                    LogMsg::JsonPatch(json_patch::Patch(vec![]))
                }
            }
        }

        fn patch_matches_scratch(
            patch: &json_patch::Patch,
            scratch_id: Uuid,
            scratch_type_str: &str,
        ) -> bool {
            let Some(op) = patch.0.first() else {
                return false;
            };
            if op.path() != "/scratch" {
                return false;
            }

            let value = match op {
                json_patch::PatchOperation::Add(a) => Some(&a.value),
                json_patch::PatchOperation::Replace(r) => Some(&r.value),
                json_patch::PatchOperation::Remove(_) => None,
                _ => None,
            };

            let id_str = scratch_id.to_string();
            value.is_some_and(|v| {
                let id_matches = v.get("id").and_then(|v| v.as_str()) == Some(&id_str);
                let type_matches = v
                    .get("payload")
                    .and_then(|p| p.get("type"))
                    .and_then(|t| t.as_str())
                    == Some(scratch_type_str);
                id_matches && type_matches
            })
        }

        async fn load_scratch_snapshot(
            db_pool: &DbPool,
            scratch_id: Uuid,
            scratch_type: &db::models::scratch::ScratchType,
        ) -> LogMsg {
            let scratch = match Scratch::find_by_id(db_pool, scratch_id, scratch_type).await {
                Ok(scratch) => scratch,
                Err(e) => {
                    tracing::warn!(
                        scratch_id = %scratch_id,
                        scratch_type = %scratch_type,
                        error = %e,
                        "Failed to load scratch, treating as empty"
                    );
                    None
                }
            };

            build_scratch_snapshot(scratch)
        }

        let (history, receiver, meta) = self.msg_store.subscribe_sequenced_from(after_seq);
        let snapshot_seq = snapshot_seq_from_meta(meta);
        let needs_snapshot = after_seq.is_none() || !can_resume_from(after_seq.unwrap_or(0), meta);

        let scratch_type = *scratch_type;
        let scratch_type_str = scratch_type.to_string();

        let mut initial_msgs: Vec<SequencedLogMsg> = Vec::new();
        let initial_last_seq: u64;

        if needs_snapshot {
            initial_msgs.push(SequencedLogMsg {
                seq: snapshot_seq,
                msg: load_scratch_snapshot(&self.db.pool, scratch_id, &scratch_type).await,
            });
            initial_last_seq = snapshot_seq;
        } else {
            for item in history {
                match item.msg {
                    LogMsg::JsonPatch(patch) => {
                        if patch_matches_scratch(&patch, scratch_id, &scratch_type_str) {
                            initial_msgs.push(SequencedLogMsg {
                                seq: item.seq,
                                msg: LogMsg::JsonPatch(patch),
                            });
                        }
                    }
                    other => initial_msgs.push(SequencedLogMsg {
                        seq: item.seq,
                        msg: other,
                    }),
                }
            }
            initial_last_seq = initial_msgs
                .last()
                .map(|msg| msg.seq)
                .unwrap_or(after_seq.unwrap_or(0));
        }

        let db_pool = self.db.pool.clone();
        let msg_store = Arc::clone(&self.msg_store);
        let last_seq = Arc::new(AtomicU64::new(initial_last_seq));

        let filtered_stream = BroadcastStream::new(receiver).filter_map(move |msg_result| {
            let db_pool = db_pool.clone();
            let msg_store = Arc::clone(&msg_store);
            let last_seq = Arc::clone(&last_seq);
            let scratch_type_str = scratch_type_str.clone();
            async move {
                match msg_result {
                    Ok(item) => {
                        if item.seq <= last_seq.load(Ordering::Relaxed) {
                            return None;
                        }

                        match item.msg {
                            LogMsg::JsonPatch(patch) => {
                                if patch_matches_scratch(&patch, scratch_id, &scratch_type_str) {
                                    last_seq.store(item.seq, Ordering::Relaxed);
                                    return Some(Ok(SequencedLogMsg {
                                        seq: item.seq,
                                        msg: LogMsg::JsonPatch(patch),
                                    }));
                                }
                                None
                            }
                            other => {
                                last_seq.store(item.seq, Ordering::Relaxed);
                                Some(Ok(SequencedLogMsg {
                                    seq: item.seq,
                                    msg: other,
                                }))
                            }
                        }
                    }
                    Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                        let watermark = msg_store.max_seq().unwrap_or(0);
                        tracing::warn!(
                            skipped = skipped,
                            watermark = watermark,
                            "scratch stream lagged; resyncing snapshot"
                        );
                        last_seq.store(watermark, Ordering::Relaxed);
                        Some(Ok(SequencedLogMsg {
                            seq: watermark,
                            msg: load_scratch_snapshot(&db_pool, scratch_id, &scratch_type).await,
                        }))
                    }
                }
            }
        });

        let initial_stream =
            futures::stream::iter(initial_msgs.into_iter().map(Ok::<_, std::io::Error>));
        Ok(initial_stream.chain(filtered_stream).boxed())
    }
}
