pub mod executor_approvals;

use std::{sync::Arc, time::Duration as StdDuration};

use dashmap::DashMap;
use db::{
    DbErr,
    models::{
        approval as approval_model,
        execution_process::ExecutionProcess,
        task::{Task, TaskStatus},
    },
};
use executors::{
    approvals::ToolCallMetadata,
    logs::{
        NormalizedEntry, NormalizedEntryType, ToolStatus,
        utils::patch::{ConversationPatch, extract_normalized_entry_from_patch},
    },
};
use futures::future::{BoxFuture, FutureExt, Shared};
use thiserror::Error;
use tokio::sync::{RwLock, broadcast, oneshot};
use utils::{
    approvals::{ApprovalRequest, ApprovalResponse, ApprovalStatus},
    log_msg::LogMsg,
    msg_store::MsgStore,
};
use uuid::Uuid;

#[derive(Debug)]
struct PendingApproval {
    execution_process_id: Uuid,
    tool_call_id: String,
    entry_index: Option<usize>,
    entry: Option<NormalizedEntry>,
    response_tx: oneshot::Sender<ApprovalStatus>,
    waiter: ApprovalWaiter,
}

type ApprovalWaiter = Shared<BoxFuture<'static, ApprovalStatus>>;

#[derive(Debug)]
pub struct ToolContext {
    pub tool_name: String,
    pub execution_process_id: Uuid,
}

#[derive(Clone)]
pub struct Approvals {
    pending: Arc<DashMap<String, PendingApproval>>,
    msg_stores: Arc<RwLock<std::collections::HashMap<Uuid, Arc<MsgStore>>>>,
    created_tx: broadcast::Sender<ApprovalRequest>,
}

#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("approval request not found")]
    NotFound,
    #[error("no executor session found for session_id: {0}")]
    NoExecutorSession(String),
    #[error("corresponding tool use entry not found for approval request")]
    NoToolUseEntry,
    #[error(transparent)]
    Custom(#[from] anyhow::Error),
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl Approvals {
    pub fn new(msg_stores: Arc<RwLock<std::collections::HashMap<Uuid, Arc<MsgStore>>>>) -> Self {
        let (created_tx, _) = broadcast::channel(256);
        Self {
            pending: Arc::new(DashMap::new()),
            msg_stores,
            created_tx,
        }
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn subscribe_created(&self) -> broadcast::Receiver<ApprovalRequest> {
        self.created_tx.subscribe()
    }

    pub async fn create_with_waiter(
        &self,
        pool: &db::DbPool,
        request: ApprovalRequest,
    ) -> Result<(ApprovalRequest, ApprovalWaiter), ApprovalError> {
        let mut request = request;

        // If we already have a pending approval for this (execution_process_id, tool_call_id),
        // reuse it so we don't create duplicate approvals for the same tool call.
        if let Some(existing) = approval_model::find_pending_by_execution_tool_call(
            pool,
            request.execution_process_id,
            &request.tool_call_id,
        )
        .await?
        {
            request.id = existing.id;
            request.created_at = existing.created_at;
            request.timeout_at = existing.timeout_at;
        } else {
            // Persist the approval. This is the source of truth for list/respond and for restart recovery.
            let approval_id = Uuid::parse_str(&request.id).map_err(|err| {
                ApprovalError::Custom(anyhow::anyhow!(
                    "Invalid approval id '{}': {}",
                    request.id,
                    err
                ))
            })?;

            let ctx = ExecutionProcess::load_context(pool, request.execution_process_id).await?;
            let attempt_id = ctx.workspace.id;

            approval_model::insert_pending(
                pool,
                approval_id,
                attempt_id,
                request.execution_process_id,
                request.tool_name.clone(),
                request.tool_input.clone(),
                request.tool_call_id.clone(),
                request.created_at,
                request.timeout_at,
            )
            .await?;
        }

        let req_id = request.id.clone();

        if let Some(existing) = self.pending.get(&req_id) {
            return Ok((request, existing.waiter.clone()));
        }

        let (tx, rx) = oneshot::channel();
        let waiter: ApprovalWaiter = rx
            .map(|result| result.unwrap_or(ApprovalStatus::TimedOut))
            .boxed()
            .shared();

        if let Some(store) = self.msg_store_by_id(&request.execution_process_id).await {
            // Find the matching tool use entry by name and input
            let matching_tool = find_matching_tool_use(store.clone(), &request.tool_call_id);

            if let Some((idx, matching_tool)) = matching_tool {
                let approval_entry = matching_tool
                    .with_tool_status(ToolStatus::PendingApproval {
                        approval_id: req_id.clone(),
                        requested_at: request.created_at,
                        timeout_at: request.timeout_at,
                    })
                    .ok_or(ApprovalError::NoToolUseEntry)?;
                store.push_patch(ConversationPatch::replace(idx, approval_entry));

                self.pending.insert(
                    req_id.clone(),
                    PendingApproval {
                        execution_process_id: request.execution_process_id,
                        tool_call_id: request.tool_call_id.clone(),
                        entry_index: Some(idx),
                        entry: Some(matching_tool),
                        response_tx: tx,
                        waiter: waiter.clone(),
                    },
                );
                tracing::debug!(
                    "Created approval {} for tool '{}' at entry index {}",
                    req_id,
                    request.tool_name,
                    idx
                );
            } else {
                tracing::warn!(
                    "No matching tool use entry found for approval request: tool='{}', execution_process_id={}",
                    request.tool_name,
                    request.execution_process_id
                );
                self.pending.insert(
                    req_id.clone(),
                    PendingApproval {
                        execution_process_id: request.execution_process_id,
                        tool_call_id: request.tool_call_id.clone(),
                        entry_index: None,
                        entry: None,
                        response_tx: tx,
                        waiter: waiter.clone(),
                    },
                );
            }
        } else {
            tracing::warn!(
                "No msg_store found for execution_process_id: {}",
                request.execution_process_id
            );
            self.pending.insert(
                req_id.clone(),
                PendingApproval {
                    execution_process_id: request.execution_process_id,
                    tool_call_id: request.tool_call_id.clone(),
                    entry_index: None,
                    entry: None,
                    response_tx: tx,
                    waiter: waiter.clone(),
                },
            );
        }

        let _ = self.created_tx.send(request.clone());
        self.spawn_timeout_watcher(
            pool.clone(),
            req_id.clone(),
            request.timeout_at,
            waiter.clone(),
        );
        Ok((request, waiter))
    }

    #[tracing::instrument(skip(self, id, req))]
    pub async fn respond(
        &self,
        pool: &db::DbPool,
        id: &str,
        req: ApprovalResponse,
    ) -> Result<(ApprovalStatus, ToolContext), ApprovalError> {
        self.respond_with_client_id(pool, id, req, None).await
    }

    #[tracing::instrument(skip(self, id, req, responded_by_client_id))]
    pub async fn respond_with_client_id(
        &self,
        pool: &db::DbPool,
        id: &str,
        req: ApprovalResponse,
        responded_by_client_id: Option<String>,
    ) -> Result<(ApprovalStatus, ToolContext), ApprovalError> {
        let approval_uuid = Uuid::parse_str(id).map_err(|_| ApprovalError::NotFound)?;

        let Some(approval) = approval_model::get_by_id(pool, approval_uuid).await? else {
            return Err(ApprovalError::NotFound);
        };

        if approval.execution_process_id != req.execution_process_id {
            return Err(ApprovalError::Custom(anyhow::anyhow!(
                "execution_process_id mismatch for approval: expected {}, got {}",
                approval.execution_process_id,
                req.execution_process_id
            )));
        }

        let tool_ctx = ToolContext {
            tool_name: approval.tool_name.clone(),
            execution_process_id: approval.execution_process_id,
        };

        // Idempotent behavior: if the approval is already completed, return its status.
        // Otherwise persist the response and unblock any waiter.
        let final_status = if matches!(approval.status, ApprovalStatus::Pending) {
            let updated = approval_model::respond(
                pool,
                approval_uuid,
                req.status.clone(),
                responded_by_client_id,
            )
            .await?;

            if let Some((_, pending)) = self.pending.remove(id) {
                let _ = pending.response_tx.send(updated.status.clone());
            }

            updated.status
        } else {
            approval.status
        };

        self.try_update_tool_entry_status(
            tool_ctx.execution_process_id,
            &approval.tool_call_id,
            &final_status,
        )
        .await;

        // If approved or denied, and task is still InReview, move back to InProgress
        if matches!(
            final_status,
            ApprovalStatus::Approved | ApprovalStatus::Denied { .. }
        ) && let Ok(ctx) =
            ExecutionProcess::load_context(pool, tool_ctx.execution_process_id).await
            && ctx.task.status == TaskStatus::InReview
            && let Err(e) = Task::update_status(pool, ctx.task.id, TaskStatus::InProgress).await
        {
            tracing::warn!(
                "Failed to update task status to InProgress after approval response: {}",
                e
            );
        }

        Ok((final_status, tool_ctx))
    }

    pub async fn get_approval(
        &self,
        pool: &db::DbPool,
        id: &str,
    ) -> Result<approval_model::Approval, ApprovalError> {
        let approval_uuid = Uuid::parse_str(id).map_err(|_| ApprovalError::NotFound)?;
        approval_model::get_by_id(pool, approval_uuid)
            .await?
            .ok_or(ApprovalError::NotFound)
    }

    pub async fn list_approvals_by_attempt(
        &self,
        pool: &db::DbPool,
        attempt_id: Uuid,
        status: Option<&str>,
        limit: u64,
        cursor: Option<i64>,
    ) -> Result<(Vec<approval_model::Approval>, Option<i64>), ApprovalError> {
        Ok(approval_model::list_by_attempt(pool, attempt_id, status, limit, cursor).await?)
    }

    async fn try_update_tool_entry_status(
        &self,
        execution_process_id: Uuid,
        tool_call_id: &str,
        approval_status: &ApprovalStatus,
    ) {
        let Some(store) = self.msg_store_by_id(&execution_process_id).await else {
            tracing::warn!(
                "No msg_store found for execution_process_id: {}",
                execution_process_id
            );
            return;
        };

        let status = match ToolStatus::from_approval_status(approval_status) {
            Some(s) => s,
            None => {
                tracing::warn!("Invalid approval status while updating tool entry");
                return;
            }
        };

        if let Some((idx, entry)) = find_tool_use_by_call_id(store.clone(), tool_call_id) {
            if let Some(updated_entry) = entry.with_tool_status(status) {
                store.push_patch(ConversationPatch::replace(idx, updated_entry));
            } else {
                tracing::warn!(
                    "Approval '{}' completed but couldn't update tool status (no tool-use entry).",
                    tool_call_id
                );
            }
        } else {
            tracing::warn!(
                "No matching tool use entry found while responding to approval: execution_process_id={}, tool_call_id={}",
                execution_process_id,
                tool_call_id
            );
        }
    }

    #[tracing::instrument(skip(self, pool, id, timeout_at, waiter))]
    fn spawn_timeout_watcher(
        &self,
        pool: db::DbPool,
        id: String,
        timeout_at: chrono::DateTime<chrono::Utc>,
        waiter: ApprovalWaiter,
    ) {
        let pending = self.pending.clone();
        let msg_stores = self.msg_stores.clone();

        let now = chrono::Utc::now();
        let to_wait = (timeout_at - now)
            .to_std()
            .unwrap_or_else(|_| StdDuration::from_secs(0));
        let deadline = tokio::time::Instant::now() + to_wait;

        tokio::spawn(async move {
            let status = tokio::select! {
                biased;

                resolved = waiter.clone() => resolved,
                _ = tokio::time::sleep_until(deadline) => ApprovalStatus::TimedOut,
            };

            let is_timeout = matches!(&status, ApprovalStatus::TimedOut);

            if is_timeout {
                if let Ok(approval_uuid) = Uuid::parse_str(&id)
                    && let Ok(Some(current)) = approval_model::get_by_id(&pool, approval_uuid).await
                    && matches!(current.status, ApprovalStatus::Pending)
                {
                    let _ = approval_model::respond(
                        &pool,
                        approval_uuid,
                        ApprovalStatus::TimedOut,
                        None,
                    )
                    .await;
                }
            }

            if is_timeout && let Some((_, pending_approval)) = pending.remove(&id) {
                if pending_approval.response_tx.send(status.clone()).is_err() {
                    tracing::debug!("approval '{}' timeout notification receiver dropped", id);
                }

                let store = {
                    let map = msg_stores.read().await;
                    map.get(&pending_approval.execution_process_id).cloned()
                };

                if let Some(store) = store {
                    if let Some((idx, entry)) = pending_approval
                        .entry_index
                        .zip(pending_approval.entry.clone())
                    {
                        if let Some(updated_entry) = entry.with_tool_status(ToolStatus::TimedOut) {
                            store.push_patch(ConversationPatch::replace(idx, updated_entry));
                        }
                    } else if let Some((idx, entry)) =
                        find_tool_use_by_call_id(store.clone(), &pending_approval.tool_call_id)
                        && let Some(updated_entry) = entry.with_tool_status(ToolStatus::TimedOut)
                    {
                        store.push_patch(ConversationPatch::replace(idx, updated_entry));
                    }
                } else {
                    tracing::warn!(
                        "No msg_store found for execution_process_id: {}",
                        pending_approval.execution_process_id
                    );
                }
            }
        });
    }

    async fn msg_store_by_id(&self, execution_process_id: &Uuid) -> Option<Arc<MsgStore>> {
        let map = self.msg_stores.read().await;
        map.get(execution_process_id).cloned()
    }
}

pub(crate) async fn ensure_task_in_review(pool: &db::DbPool, execution_process_id: Uuid) {
    if let Ok(ctx) = ExecutionProcess::load_context(pool, execution_process_id).await
        && ctx.task.status == TaskStatus::InProgress
        && let Err(e) = Task::update_status(pool, ctx.task.id, TaskStatus::InReview).await
    {
        tracing::warn!(
            "Failed to update task status to InReview for approval request: {}",
            e
        );
    }
}

/// Find a matching tool use entry that hasn't been assigned to an approval yet
/// Matches by tool call id from tool metadata
fn find_matching_tool_use(
    store: Arc<MsgStore>,
    tool_call_id: &str,
) -> Option<(usize, NormalizedEntry)> {
    let history = store.get_history();

    // Single loop through history
    for msg in history.iter().rev() {
        if let LogMsg::JsonPatch(patch) = msg
            && let Some((idx, entry)) = extract_normalized_entry_from_patch(patch)
            && let NormalizedEntryType::ToolUse { status, .. } = &entry.entry_type
        {
            // Only match tools that are in Created state
            if !matches!(status, ToolStatus::Created) {
                continue;
            }

            // Match by tool call id from metadata
            if let Some(metadata) = &entry.metadata
                && let Ok(ToolCallMetadata {
                    tool_call_id: entry_call_id,
                    ..
                }) = serde_json::from_value::<ToolCallMetadata>(metadata.clone())
                && entry_call_id == tool_call_id
            {
                tracing::debug!(
                    "Matched tool use entry at index {idx} for tool call id '{tool_call_id}'"
                );
                return Some((idx, entry));
            }
        }
    }

    None
}

fn find_tool_use_by_call_id(
    store: Arc<MsgStore>,
    tool_call_id: &str,
) -> Option<(usize, NormalizedEntry)> {
    let history = store.get_history();

    for msg in history.iter().rev() {
        if let LogMsg::JsonPatch(patch) = msg
            && let Some((idx, entry)) = extract_normalized_entry_from_patch(patch)
            && let NormalizedEntryType::ToolUse { .. } = &entry.entry_type
            && let Some(metadata) = &entry.metadata
            && let Ok(ToolCallMetadata {
                tool_call_id: entry_call_id,
                ..
            }) = serde_json::from_value::<ToolCallMetadata>(metadata.clone())
            && entry_call_id == tool_call_id
        {
            return Some((idx, entry));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use db::{
        DBService,
        models::{
            execution_process::{CreateExecutionProcess, ExecutionProcess},
            execution_process_repo_state::CreateExecutionProcessRepoState,
            project::Project,
            repo::Repo,
            session::{CreateSession, Session},
            task::Task,
            workspace::{CreateWorkspace, Workspace},
            workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
        },
        types::ExecutionProcessRunReason,
    };
    use executors::logs::{ActionType, NormalizedEntry, NormalizedEntryType, ToolStatus};
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use utils::{approvals::CreateApprovalRequest, msg_store::MsgStore};

    use super::*;

    async fn setup_db() -> DBService {
        let pool = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&pool, None).await.unwrap();
        DBService { pool }
    }

    async fn seed_execution_context(db: &DBService) -> (Uuid, Uuid) {
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

        let repo = Repo::find_or_create(
            &db.pool,
            std::path::Path::new("/tmp/vibe-kanban-test-repo"),
            "Test repo",
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        let workspace = Workspace::create(
            &db.pool,
            &CreateWorkspace {
                branch: "test-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        WorkspaceRepo::create_many(
            &db.pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".to_string(),
            }],
        )
        .await
        .unwrap();

        let session = Session::create(
            &db.pool,
            &CreateSession {
                executor: Some("CLAUDE_CODE".to_string()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap();

        let exec_id = Uuid::new_v4();
        let _process = ExecutionProcess::create(
            &db.pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: executors::actions::ExecutorAction::new(
                    executors::actions::ExecutorActionType::ScriptRequest(
                        executors::actions::script::ScriptRequest {
                            language: executors::actions::script::ScriptRequestLanguage::Bash,
                            script: "echo hello".to_string(),
                            context: executors::actions::script::ScriptContext::SetupScript,
                            working_dir: None,
                        },
                    ),
                    None,
                ),
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            exec_id,
            &[CreateExecutionProcessRepoState {
                repo_id: repo.id,
                before_head_commit: None,
                after_head_commit: None,
                merge_commit: None,
            }],
        )
        .await
        .unwrap();

        (workspace.id, exec_id)
    }

    fn create_tool_use_entry(
        tool_name: &str,
        file_path: &str,
        id: &str,
        status: ToolStatus,
    ) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: tool_name.to_string(),
                action_type: ActionType::FileRead {
                    path: file_path.to_string(),
                },
                status,
            },
            content: format!("Reading {file_path}"),
            metadata: Some(
                serde_json::to_value(ToolCallMetadata {
                    tool_call_id: id.to_string(),
                })
                .unwrap(),
            ),
        }
    }

    #[test]
    fn test_parallel_tool_call_approval_matching() {
        let store = Arc::new(MsgStore::new());

        // Setup: Simulate 3 parallel Read tool calls with different files
        let read_foo = create_tool_use_entry("Read", "foo.rs", "foo-id", ToolStatus::Created);
        let read_bar = create_tool_use_entry("Read", "bar.rs", "bar-id", ToolStatus::Created);
        let read_baz = create_tool_use_entry("Read", "baz.rs", "baz-id", ToolStatus::Created);

        store.push_patch(
            executors::logs::utils::patch::ConversationPatch::add_normalized_entry(0, read_foo),
        );
        store.push_patch(
            executors::logs::utils::patch::ConversationPatch::add_normalized_entry(1, read_bar),
        );
        store.push_patch(
            executors::logs::utils::patch::ConversationPatch::add_normalized_entry(2, read_baz),
        );

        let (idx_foo, _) =
            find_matching_tool_use(store.clone(), "foo-id").expect("Should match foo.rs");
        let (idx_bar, _) =
            find_matching_tool_use(store.clone(), "bar-id").expect("Should match bar.rs");
        let (idx_baz, _) =
            find_matching_tool_use(store.clone(), "baz-id").expect("Should match baz.rs");

        assert_eq!(idx_foo, 0, "foo.rs should match first entry");
        assert_eq!(idx_bar, 1, "bar.rs should match second entry");
        assert_eq!(idx_baz, 2, "baz.rs should match third entry");

        // Test 2: Already pending tools are skipped
        let read_pending = create_tool_use_entry(
            "Read",
            "pending.rs",
            "pending-id",
            ToolStatus::PendingApproval {
                approval_id: "test-id".to_string(),
                requested_at: chrono::Utc::now(),
                timeout_at: chrono::Utc::now(),
            },
        );
        store.push_patch(
            executors::logs::utils::patch::ConversationPatch::add_normalized_entry(3, read_pending),
        );

        assert!(
            find_matching_tool_use(store.clone(), "pending-id").is_none(),
            "Should not match tools in PendingApproval state"
        );

        // Test 3: Wrong tool id returns None
        assert!(
            find_matching_tool_use(store.clone(), "wrong-id").is_none(),
            "Should not match different tool ids"
        );
    }

    #[tokio::test]
    async fn approval_create_list_get_and_respond_unblocks_waiter() {
        let db = setup_db().await;
        let (attempt_id, execution_process_id) = seed_execution_context(&db).await;

        let msg_stores = Arc::new(RwLock::new(
            std::collections::HashMap::<Uuid, Arc<MsgStore>>::new(),
        ));
        let approvals = Approvals::new(msg_stores);

        let request = ApprovalRequest::from_create(
            CreateApprovalRequest {
                tool_name: "Read".to_string(),
                tool_input: serde_json::json!({"path": "README.md"}),
                tool_call_id: "tool-call-1".to_string(),
            },
            execution_process_id,
        );

        let (request, waiter) = approvals
            .create_with_waiter(&db.pool, request)
            .await
            .unwrap();

        let (pending, _) = approvals
            .list_approvals_by_attempt(&db.pool, attempt_id, Some("pending"), 50, None)
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, request.id);

        let fetched = approvals.get_approval(&db.pool, &request.id).await.unwrap();
        assert_eq!(fetched.id, request.id);

        let (status, _ctx) = approvals
            .respond(
                &db.pool,
                &request.id,
                ApprovalResponse {
                    execution_process_id,
                    status: ApprovalStatus::Approved,
                },
            )
            .await
            .unwrap();
        assert!(matches!(status, ApprovalStatus::Approved));

        let resolved = waiter.await;
        assert!(matches!(resolved, ApprovalStatus::Approved));
    }

    #[tokio::test]
    async fn approval_can_be_listed_and_responded_after_service_restart() {
        let db = setup_db().await;
        let (attempt_id, execution_process_id) = seed_execution_context(&db).await;

        let msg_stores = Arc::new(RwLock::new(
            std::collections::HashMap::<Uuid, Arc<MsgStore>>::new(),
        ));
        let approvals = Approvals::new(msg_stores.clone());

        let request = ApprovalRequest::from_create(
            CreateApprovalRequest {
                tool_name: "Write".to_string(),
                tool_input: serde_json::json!({"path": "src/lib.rs"}),
                tool_call_id: "tool-call-restart".to_string(),
            },
            execution_process_id,
        );

        let (request, _waiter) = approvals
            .create_with_waiter(&db.pool, request)
            .await
            .unwrap();

        // "Restart" by constructing a new service with empty in-memory pending state.
        let approvals_after_restart = Approvals::new(msg_stores);

        let (pending, _) = approvals_after_restart
            .list_approvals_by_attempt(&db.pool, attempt_id, Some("pending"), 50, None)
            .await
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, request.id);

        let (status, _ctx) = approvals_after_restart
            .respond(
                &db.pool,
                &request.id,
                ApprovalResponse {
                    execution_process_id,
                    status: ApprovalStatus::Denied {
                        reason: Some("nope".to_string()),
                    },
                },
            )
            .await
            .unwrap();
        assert!(matches!(status, ApprovalStatus::Denied { .. }));

        let updated = approvals_after_restart
            .get_approval(&db.pool, &request.id)
            .await
            .unwrap();
        assert!(matches!(updated.status, ApprovalStatus::Denied { .. }));
    }
}
