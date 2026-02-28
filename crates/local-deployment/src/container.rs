use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use command_group::AsyncGroupChild;
use db::{
    DBService, DbErr,
    models::{
        coding_agent_turn::CodingAgentTurn,
        execution_process::{
            ExecutionContext, ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus,
        },
        execution_process_logs::ExecutionProcessLogs,
        execution_process_repo_state::ExecutionProcessRepoState,
        project_repo::ProjectRepo,
        repo::Repo,
        scratch::{DraftFollowUpData, Scratch, ScratchType},
        task::{Task, TaskStatus},
        workspace::Workspace,
        workspace_repo::WorkspaceRepo,
    },
};
use deployment::DeploymentError;
use executors::{
    actions::{
        Executable, ExecutorAction, ExecutorActionType,
        coding_agent_follow_up::CodingAgentFollowUpRequest,
        coding_agent_initial::CodingAgentInitialRequest,
    },
    approvals::{ExecutorApprovalService, NoopExecutorApprovalService},
    auto_retry::AutoRetryConfig,
    env::ExecutionEnv,
    executors::{BaseCodingAgent, ExecutorExitResult, ExecutorExitSignal, InterruptSender},
    logs::{
        NormalizedEntry, NormalizedEntryType,
        utils::{
            ConversationPatch, EntryIndexProvider, patch::extract_normalized_entry_from_patch,
        },
    },
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use futures::{FutureExt, StreamExt, TryStreamExt, stream::select};
use serde_json::json;
use services::services::{
    approvals::{Approvals, executor_approvals::ExecutorApprovalBridge},
    config::Config,
    container::{ContainerError, ContainerRef, ContainerService, DiffStreamOptions},
    diff_stream::{self, DiffStreamHandle},
    git::{Commit, GitCli, GitCommitOptions, GitService, WorktreeResetOptions},
    image::ImageService,
    notification::NotificationService,
    queued_message::QueuedMessageService,
    workspace_manager::{RepoWorkspaceInput, WorkspaceManager},
};
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_util::io::ReaderStream;
use utils::{
    diff::DiffSummary,
    log_msg::LogMsg,
    msg_store::MsgStore,
    text::{git_branch_id, short_uuid, truncate_to_char_boundary},
};
use uuid::Uuid;

use crate::{command, copy};

#[derive(Debug, Clone, Copy)]
struct AutoRetryState {
    attempt: u32,
}

#[derive(Clone, Default)]
struct FinalizationTracker {
    in_progress: Arc<RwLock<HashSet<Uuid>>>,
}

impl FinalizationTracker {
    async fn begin(&self, execution_process_id: Uuid) -> bool {
        self.in_progress.write().await.insert(execution_process_id)
    }

    async fn end(&self, execution_process_id: Uuid) {
        self.in_progress.write().await.remove(&execution_process_id);
    }
}

const WORKSPACE_EXPIRED_TTL_ENV: &str = "VK_WORKSPACE_EXPIRED_TTL_SECS";
const WORKSPACE_CLEANUP_INTERVAL_ENV: &str = "VK_WORKSPACE_CLEANUP_INTERVAL_SECS";
const DISABLE_WORKSPACE_EXPIRED_CLEANUP_ENV: &str = "DISABLE_WORKSPACE_EXPIRED_CLEANUP";

const DEFAULT_WORKSPACE_EXPIRED_TTL_SECS: i64 = 60 * 60 * 72; // 72 hours
const DEFAULT_WORKSPACE_CLEANUP_INTERVAL_SECS: u64 = 60 * 30; // 30 minutes

const MIN_WORKSPACE_EXPIRED_TTL_SECS: i64 = 60; // 1 minute
const MIN_WORKSPACE_CLEANUP_INTERVAL_SECS: u64 = 10; // 10 seconds

fn read_env_u64(name: &str, default: u64, min: u64) -> u64 {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(value) if value >= min => value,
            Ok(value) => {
                tracing::warn!(
                    "{} set to {} (min {}); clamping to {}",
                    name,
                    value,
                    min,
                    min
                );
                min
            }
            Err(err) => {
                tracing::warn!(
                    "Invalid {}='{}': {}. Using default {}",
                    name,
                    raw,
                    err,
                    default
                );
                default
            }
        },
        Err(_) => default,
    }
}

fn read_env_i64(name: &str, default: i64, min: i64) -> i64 {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().parse::<i64>() {
            Ok(value) if value >= min => value,
            Ok(value) => {
                tracing::warn!(
                    "{} set to {} (min {}); clamping to {}",
                    name,
                    value,
                    min,
                    min
                );
                min
            }
            Err(err) => {
                tracing::warn!(
                    "Invalid {}='{}': {}. Using default {}",
                    name,
                    raw,
                    err,
                    default
                );
                default
            }
        },
        Err(_) => default,
    }
}

#[derive(Clone)]
pub struct LocalContainerService {
    db: DBService,
    child_store: Arc<RwLock<HashMap<Uuid, Arc<RwLock<AsyncGroupChild>>>>>,
    interrupt_senders: Arc<RwLock<HashMap<Uuid, InterruptSender>>>,
    msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
    auto_retry_states: Arc<RwLock<HashMap<Uuid, AutoRetryState>>>,
    finalization_tracker: FinalizationTracker,
    config: Arc<RwLock<Config>>,
    git: GitService,
    image_service: ImageService,
    approvals: Approvals,
    queued_message_service: QueuedMessageService,
    notification_service: NotificationService,
}

impl LocalContainerService {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: DBService,
        msg_stores: Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>,
        config: Arc<RwLock<Config>>,
        git: GitService,
        image_service: ImageService,
        approvals: Approvals,
        queued_message_service: QueuedMessageService,
    ) -> Self {
        let child_store = Arc::new(RwLock::new(HashMap::new()));
        let interrupt_senders = Arc::new(RwLock::new(HashMap::new()));
        let auto_retry_states = Arc::new(RwLock::new(HashMap::new()));
        let finalization_tracker = FinalizationTracker::default();
        let notification_service = NotificationService::new(config.clone());

        let container = LocalContainerService {
            db,
            child_store,
            interrupt_senders,
            msg_stores,
            auto_retry_states,
            finalization_tracker,
            config,
            git,
            image_service,
            approvals,
            queued_message_service,
            notification_service,
        };

        container.spawn_workspace_cleanup().await;

        container
    }

    pub async fn get_child_from_store(&self, id: &Uuid) -> Option<Arc<RwLock<AsyncGroupChild>>> {
        let map = self.child_store.read().await;
        map.get(id).cloned()
    }

    pub async fn add_child_to_store(&self, id: Uuid, exec: AsyncGroupChild) {
        let mut map = self.child_store.write().await;
        map.insert(id, Arc::new(RwLock::new(exec)));
    }

    pub async fn remove_child_from_store(&self, id: &Uuid) {
        let mut map = self.child_store.write().await;
        map.remove(id);
    }

    async fn add_interrupt_sender(&self, id: Uuid, sender: InterruptSender) {
        let mut map = self.interrupt_senders.write().await;
        map.insert(id, sender);
    }

    async fn take_interrupt_sender(&self, id: &Uuid) -> Option<InterruptSender> {
        let mut map = self.interrupt_senders.write().await;
        map.remove(id)
    }

    async fn begin_finalization(&self, execution_process_id: Uuid) -> bool {
        self.finalization_tracker.begin(execution_process_id).await
    }

    async fn end_finalization(&self, execution_process_id: Uuid) {
        self.finalization_tracker.end(execution_process_id).await;
    }

    pub async fn cleanup_workspace(db: &DBService, workspace: &Workspace) {
        let Some(container_ref) = &workspace.container_ref else {
            return;
        };
        let workspace_dir = PathBuf::from(container_ref);

        let repositories = WorkspaceRepo::find_repos_for_workspace(&db.pool, workspace.id)
            .await
            .unwrap_or_default();

        if repositories.is_empty() {
            tracing::warn!(
                "No repositories found for workspace {}, cleaning up workspace directory only",
                workspace.id
            );
            if workspace_dir.exists()
                && let Err(e) = tokio::fs::remove_dir_all(&workspace_dir).await
            {
                tracing::warn!("Failed to remove workspace directory: {}", e);
            }
        } else {
            WorkspaceManager::cleanup_workspace(&workspace_dir, &repositories)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to clean up workspace for workspace {}: {}",
                        workspace.id,
                        e
                    );
                });
        }

        // Clear container_ref so this workspace won't be picked up again
        let _ = Workspace::clear_container_ref(&db.pool, workspace.id).await;
    }

    pub async fn cleanup_expired_workspaces(db: &DBService) -> Result<(), DeploymentError> {
        if std::env::var(DISABLE_WORKSPACE_EXPIRED_CLEANUP_ENV).is_ok() {
            tracing::debug!(
                "Expired workspace cleanup disabled via {}",
                DISABLE_WORKSPACE_EXPIRED_CLEANUP_ENV
            );
            return Ok(());
        }

        let ttl_secs = read_env_i64(
            WORKSPACE_EXPIRED_TTL_ENV,
            DEFAULT_WORKSPACE_EXPIRED_TTL_SECS,
            MIN_WORKSPACE_EXPIRED_TTL_SECS,
        );
        let cutoff = Utc::now() - ChronoDuration::seconds(ttl_secs);

        let expired_workspaces = Workspace::find_expired_for_cleanup(&db.pool, cutoff).await?;
        if expired_workspaces.is_empty() {
            tracing::debug!("No expired workspaces found");
            return Ok(());
        }
        tracing::info!(
            "Found {} expired workspaces to clean up",
            expired_workspaces.len()
        );
        for workspace in &expired_workspaces {
            Self::cleanup_workspace(db, workspace).await;
        }
        Ok(())
    }

    pub async fn spawn_workspace_cleanup(&self) {
        let db = self.db.clone();
        let interval_secs = read_env_u64(
            WORKSPACE_CLEANUP_INTERVAL_ENV,
            DEFAULT_WORKSPACE_CLEANUP_INTERVAL_SECS,
            MIN_WORKSPACE_CLEANUP_INTERVAL_SECS,
        );
        tracing::info!(
            "Workspace cleanup interval set to {}s via {} (default {}s)",
            interval_secs,
            WORKSPACE_CLEANUP_INTERVAL_ENV,
            DEFAULT_WORKSPACE_CLEANUP_INTERVAL_SECS
        );
        let mut cleanup_interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
        WorkspaceManager::cleanup_orphan_workspaces(&self.db.pool).await;
        tokio::spawn(async move {
            loop {
                cleanup_interval.tick().await;
                tracing::info!("Starting periodic workspace cleanup...");
                Self::cleanup_expired_workspaces(&db)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to clean up expired workspaces: {}", e)
                    });
            }
        });
    }

    /// Record the current HEAD commit for each repository as the "after" state.
    /// Errors are silently ignored since this runs after the main execution completes
    /// and failure should not block process finalization.
    async fn update_after_head_commits(&self, exec_id: Uuid) {
        if let Ok(ctx) = ExecutionProcess::load_context(&self.db.pool, exec_id).await {
            let workspace_root = self.workspace_to_current_dir(&ctx.workspace);
            for repo in &ctx.repos {
                let repo_path = workspace_root.join(&repo.name);
                if let Ok(head) = self.git().get_head_info(&repo_path) {
                    let _ = ExecutionProcessRepoState::update_after_head_commit(
                        &self.db.pool,
                        exec_id,
                        repo.id,
                        &head.oid,
                    )
                    .await;
                }
            }
        }
    }

    /// Get the commit message based on the execution run reason.
    async fn get_commit_message(&self, ctx: &ExecutionContext) -> String {
        match ctx.execution_process.run_reason {
            ExecutionProcessRunReason::CodingAgent => {
                // Try to retrieve the task summary from the coding agent turn
                // otherwise fallback to default message
                match CodingAgentTurn::find_by_execution_process_id(
                    &self.db().pool,
                    ctx.execution_process.id,
                )
                .await
                {
                    Ok(Some(turn)) if turn.summary.is_some() => turn.summary.unwrap(),
                    Ok(_) => {
                        tracing::debug!(
                            "No summary found for execution process {}, using default message",
                            ctx.execution_process.id
                        );
                        format!(
                            "Commit changes from coding agent for workspace {}",
                            ctx.workspace.id
                        )
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Failed to retrieve summary for execution process {}: {}",
                            ctx.execution_process.id,
                            e
                        );
                        format!(
                            "Commit changes from coding agent for workspace {}",
                            ctx.workspace.id
                        )
                    }
                }
            }
            ExecutionProcessRunReason::CleanupScript => {
                format!("Cleanup script changes for workspace {}", ctx.workspace.id)
            }
            _ => format!(
                "Changes from execution process {}",
                ctx.execution_process.id
            ),
        }
    }

    /// Check which repos have uncommitted changes. Fails if any repo is inaccessible.
    fn check_repos_for_changes(
        &self,
        workspace_root: &Path,
        repos: &[Repo],
    ) -> Result<Vec<(Repo, PathBuf)>, ContainerError> {
        let git = GitCli::new();
        let mut repos_with_changes = Vec::new();

        for repo in repos {
            let worktree_path = workspace_root.join(&repo.name);

            match git.has_changes(&worktree_path) {
                Ok(true) => {
                    repos_with_changes.push((repo.clone(), worktree_path));
                }
                Ok(false) => {
                    tracing::debug!("No changes in repo '{}'", repo.name);
                }
                Err(e) => {
                    return Err(ContainerError::Other(anyhow!(
                        "Pre-flight check failed for repo '{}': {}",
                        repo.name,
                        e
                    )));
                }
            }
        }

        Ok(repos_with_changes)
    }

    /// Commit changes to each repo. Logs failures but continues with other repos.
    async fn commit_repos(&self, repos_with_changes: Vec<(Repo, PathBuf)>, message: &str) -> bool {
        let mut any_committed = false;
        let no_verify = self.config.read().await.git_no_verify;
        let commit_options = GitCommitOptions::new(no_verify);

        for (repo, worktree_path) in repos_with_changes {
            tracing::debug!(
                "Committing changes for repo '{}' at {:?}",
                repo.name,
                &worktree_path
            );

            match self
                .git()
                .commit_with_options(&worktree_path, message, commit_options)
            {
                Ok(true) => {
                    any_committed = true;
                    tracing::info!("Committed changes in repo '{}'", repo.name);
                }
                Ok(false) => {
                    tracing::warn!("No changes committed in repo '{}' (unexpected)", repo.name);
                }
                Err(e) => {
                    tracing::warn!("Failed to commit in repo '{}': {}", repo.name, e);
                }
            }
        }

        any_committed
    }

    fn collect_auto_retry_error_text(&self, exec_id: &Uuid) -> Option<String> {
        let msg_stores = self.msg_stores.try_read().ok()?;
        let msg_store = msg_stores.get(exec_id)?;
        let history = msg_store.get_history();

        let mut lines = Vec::new();
        for msg in history.iter() {
            if let LogMsg::JsonPatch(patch) = msg
                && let Some((_, entry)) = extract_normalized_entry_from_patch(patch)
            {
                match entry.entry_type {
                    NormalizedEntryType::ErrorMessage { .. } => {
                        lines.push(entry.content);
                    }
                    NormalizedEntryType::SystemMessage => {
                        let lowered = entry.content.to_lowercase();
                        if lowered.contains("error") || lowered.contains("failed") {
                            lines.push(entry.content);
                        }
                    }
                    _ => {}
                }
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    async fn emit_auto_retry_tip(
        &self,
        exec_id: Uuid,
        attempt: u32,
        max_attempts: u32,
        delay_seconds: u32,
    ) {
        let content =
            format!("Auto retry scheduled in {delay_seconds}s (attempt {attempt}/{max_attempts})");
        let entry = NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::SystemMessage,
            content,
            metadata: Some(json!({
                "system_tip": "auto_retry",
                "attempt": attempt,
                "max_attempts": max_attempts,
                "delay_seconds": delay_seconds,
            })),
        };

        let Some(msg_store) = self.get_msg_store_by_id(&exec_id).await else {
            return;
        };

        let index_provider = EntryIndexProvider::start_from(&msg_store);
        let patch = ConversationPatch::add_normalized_entry(index_provider.next(), entry);
        msg_store.push_patch(patch.clone());

        if let Ok(json_line) = serde_json::to_string::<LogMsg>(&LogMsg::JsonPatch(patch)) {
            let _ = ExecutionProcessLogs::append_log_line(
                &self.db.pool,
                exec_id,
                &format!("{json_line}\n"),
            )
            .await;
        }
    }

    async fn session_has_running_processes(
        &self,
        session_id: Uuid,
    ) -> Result<bool, ContainerError> {
        let processes =
            ExecutionProcess::find_by_session_id(&self.db.pool, session_id, false).await?;
        Ok(processes.iter().any(|process| {
            process.status == ExecutionProcessStatus::Running
                && process.run_reason != ExecutionProcessRunReason::DevServer
        }))
    }

    async fn restore_worktrees_to_process(
        &self,
        workspace: &Workspace,
        session_id: Uuid,
        target_process_id: Uuid,
        perform_git_reset: bool,
        force_when_dirty: bool,
    ) -> Result<(), ContainerError> {
        let repos = WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;
        let repo_states = ExecutionProcessRepoState::find_by_execution_process_id(
            &self.db.pool,
            target_process_id,
        )
        .await?;

        let container_ref = self.ensure_container_exists(workspace).await?;
        let workspace_dir = PathBuf::from(container_ref);

        let is_dirty = self
            .is_container_clean(workspace)
            .await
            .map(|is_clean| !is_clean)
            .unwrap_or(false);

        for repo in &repos {
            let repo_state = repo_states.iter().find(|s| s.repo_id == repo.id);
            let target_oid = match repo_state.and_then(|s| s.before_head_commit.clone()) {
                Some(oid) => Some(oid),
                None => {
                    ExecutionProcess::find_prev_after_head_commit(
                        &self.db.pool,
                        session_id,
                        target_process_id,
                        repo.id,
                    )
                    .await?
                }
            };

            if let Some(oid) = target_oid {
                let worktree_path = workspace_dir.join(&repo.name);
                self.git().reconcile_worktree_to_commit(
                    &worktree_path,
                    &oid,
                    WorktreeResetOptions::new(
                        perform_git_reset,
                        force_when_dirty,
                        is_dirty,
                        perform_git_reset,
                    ),
                );
            }
        }

        Ok(())
    }

    async fn start_auto_retry_follow_up(
        &self,
        ctx: &ExecutionContext,
        executor_profile_id: ExecutorProfileId,
        prompt: String,
    ) -> Result<ExecutionProcess, ContainerError> {
        let latest_agent_session_id = ExecutionProcess::find_latest_coding_agent_turn_session_id(
            &self.db.pool,
            ctx.session.id,
        )
        .await?;

        let project_repos =
            ProjectRepo::find_by_project_id_with_names(&self.db.pool, ctx.project.id).await?;
        let cleanup_action = self.cleanup_actions_for_repos(&project_repos);

        let working_dir = ctx
            .workspace
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();
        let image_paths = match self
            .image_service
            .image_path_map_for_task(ctx.task.id)
            .await
        {
            Ok(map) if !map.is_empty() => Some(map),
            Ok(_) => None,
            Err(err) => {
                tracing::warn!("Failed to resolve task image paths: {}", err);
                None
            }
        };

        let action_type = if let Some(agent_session_id) = latest_agent_session_id {
            ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                prompt: prompt.clone(),
                session_id: agent_session_id,
                executor_profile_id: executor_profile_id.clone(),
                working_dir: working_dir.clone(),
                image_paths: image_paths.clone(),
            })
        } else {
            ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                prompt,
                executor_profile_id: executor_profile_id.clone(),
                working_dir,
                image_paths,
            })
        };

        let action = ExecutorAction::new(action_type, cleanup_action.map(Box::new));

        self.start_execution(
            &ctx.workspace,
            &ctx.session,
            &action,
            &ExecutionProcessRunReason::CodingAgent,
        )
        .await
    }

    async fn perform_auto_retry(
        &self,
        failed_process_id: Uuid,
        session_id: Uuid,
        prompt: String,
        executor_profile_id: ExecutorProfileId,
        auto_retry: AutoRetryConfig,
        attempt: u32,
    ) -> Result<(), ContainerError> {
        if auto_retry.delay_seconds == 0 {
            return Ok(());
        }

        if self.session_has_running_processes(session_id).await? {
            return Ok(());
        }

        let ctx = ExecutionProcess::load_context(&self.db.pool, failed_process_id).await?;

        self.restore_worktrees_to_process(
            &ctx.workspace,
            session_id,
            failed_process_id,
            true,
            false,
        )
        .await?;

        self.try_stop(&ctx.workspace, false).await;

        let _ = ExecutionProcess::drop_at_and_after(&self.db.pool, session_id, failed_process_id)
            .await?;

        let new_process = self
            .start_auto_retry_follow_up(&ctx, executor_profile_id, prompt)
            .await?;

        let mut states = self.auto_retry_states.write().await;
        states.insert(new_process.id, AutoRetryState { attempt });

        Ok(())
    }

    async fn maybe_schedule_auto_retry(
        &self,
        ctx: &ExecutionContext,
    ) -> Result<bool, ContainerError> {
        if ctx.execution_process.run_reason != ExecutionProcessRunReason::CodingAgent {
            return Ok(false);
        }

        if ctx.execution_process.status != ExecutionProcessStatus::Failed {
            return Ok(false);
        }

        let action = ctx.execution_process.executor_action()?;
        let (executor_profile_id, prompt) = match action.typ() {
            ExecutorActionType::CodingAgentInitialRequest(request) => {
                (request.executor_profile_id.clone(), request.prompt.clone())
            }
            ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                (request.executor_profile_id.clone(), request.prompt.clone())
            }
            _ => return Ok(false),
        };

        let agent = ExecutorConfigs::get_cached().get_coding_agent_or_default(&executor_profile_id);
        let auto_retry = agent.auto_retry_config().clone();

        if !auto_retry.is_enabled() || auto_retry.delay_seconds == 0 {
            return Ok(false);
        }

        let current_attempt = {
            let states = self.auto_retry_states.read().await;
            states
                .get(&ctx.execution_process.id)
                .map(|state| state.attempt)
                .unwrap_or(0)
        };

        if current_attempt >= auto_retry.max_attempts {
            return Ok(false);
        }

        let Some(error_text) = self.collect_auto_retry_error_text(&ctx.execution_process.id) else {
            return Ok(false);
        };

        if !auto_retry.matches_error(&error_text) {
            return Ok(false);
        }

        let next_attempt = current_attempt + 1;
        self.emit_auto_retry_tip(
            ctx.execution_process.id,
            next_attempt,
            auto_retry.max_attempts,
            auto_retry.delay_seconds,
        )
        .await;

        let failed_process_id = ctx.execution_process.id;
        let session_id = ctx.session.id;
        let container = self.clone();
        let prompt = prompt.clone();
        let executor_profile_id = executor_profile_id.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(u64::from(auto_retry.delay_seconds))).await;
            if let Err(err) = container
                .perform_auto_retry(
                    failed_process_id,
                    session_id,
                    prompt,
                    executor_profile_id,
                    auto_retry,
                    next_attempt,
                )
                .await
            {
                tracing::warn!(
                    "Auto retry failed for process {}: {}",
                    failed_process_id,
                    err
                );
            }
        });

        Ok(true)
    }

    /// Spawn a background task that polls the child process for completion and
    /// cleans up the execution entry when it exits.
    pub fn spawn_exit_monitor(
        &self,
        exec_id: &Uuid,
        exit_signal: Option<ExecutorExitSignal>,
    ) -> JoinHandle<()> {
        let exec_id = *exec_id;
        let child_store = self.child_store.clone();
        let msg_stores = self.msg_stores.clone();
        let db = self.db.clone();
        let container = self.clone();

        let mut process_exit_rx = self.spawn_os_exit_watcher(exec_id);

        tokio::spawn(async move {
            let mut exit_signal_future = exit_signal
                .map(|rx| rx.boxed()) // wait for result
                .unwrap_or_else(|| std::future::pending().boxed()); // no signal, stall forever

            let status_result: std::io::Result<std::process::ExitStatus>;

            // Wait for process to exit, or exit signal from executor
            tokio::select! {
                // Exit signal with result.
                // Some coding agent processes do not automatically exit after processing the user request; instead the executor
                // signals when processing has finished to gracefully kill the process.
                exit_result = &mut exit_signal_future => {
                    // Executor signaled completion: kill group and use the provided result
                    if let Some(child_lock) = child_store.read().await.get(&exec_id).cloned() {
                        let mut child = child_lock.write().await ;
                        if let Err(err) = command::kill_process_group(&mut child).await {
                            tracing::error!("Failed to kill process group after exit signal: {} {}", exec_id, err);
                        }
                    }

                    // Map the exit result to appropriate exit status
                    status_result = match exit_result {
                        Ok(ExecutorExitResult::Success) => Ok(success_exit_status()),
                        Ok(ExecutorExitResult::Failure) => Ok(failure_exit_status()),
                        Err(_) => Ok(success_exit_status()), // Channel closed, assume success
                    };
                }
                // Process exit
                exit_status_result = &mut process_exit_rx => {
                    status_result = exit_status_result.unwrap_or_else(|e| Err(std::io::Error::other(e)));
                }
            }

            let (exit_code, status) = match status_result {
                Ok(exit_status) => {
                    let code = exit_status.code().unwrap_or(-1) as i64;
                    let status = if exit_status.success() {
                        ExecutionProcessStatus::Completed
                    } else {
                        ExecutionProcessStatus::Failed
                    };
                    (Some(code), status)
                }
                Err(_) => (None, ExecutionProcessStatus::Failed),
            };

            if !ExecutionProcess::was_stopped(&db.pool, exec_id).await
                && let Err(e) =
                    ExecutionProcess::update_completion(&db.pool, exec_id, status, exit_code).await
            {
                tracing::error!("Failed to update execution process completion: {}", e);
            }

            let owns_finalization = container.begin_finalization(exec_id).await;
            if !owns_finalization {
                tracing::debug!(
                    "Skipping exit monitor finalization for process {} because another finalizer is active",
                    exec_id
                );
            } else {
                if let Ok(ctx) = ExecutionProcess::load_context(&db.pool, exec_id).await {
                    // Avoid double-finalizing the same execution (duplicate notifications).
                    let mut finalized = false;

                    // Update executor session summary if available
                    if let Err(e) = container.update_executor_session_summary(&exec_id).await {
                        tracing::warn!("Failed to update executor session summary: {}", e);
                    }

                    let success = matches!(
                        ctx.execution_process.status,
                        ExecutionProcessStatus::Completed
                    ) && exit_code == Some(0);

                    let cleanup_done = matches!(
                        ctx.execution_process.run_reason,
                        ExecutionProcessRunReason::CleanupScript
                    ) && !matches!(
                        ctx.execution_process.status,
                        ExecutionProcessStatus::Running
                    );

                    if success || cleanup_done {
                        // Commit changes (if any) and get feedback about whether changes were made
                        let changes_committed = match container.try_commit_changes(&ctx).await {
                            Ok(committed) => committed,
                            Err(e) => {
                                tracing::error!("Failed to commit changes after execution: {}", e);
                                // Treat commit failures as if changes were made to be safe
                                true
                            }
                        };

                        let should_start_next = if matches!(
                            ctx.execution_process.run_reason,
                            ExecutionProcessRunReason::CodingAgent
                        ) {
                            changes_committed
                        } else {
                            true
                        };

                        if should_start_next {
                            // If the process exited successfully, start the next action
                            if let Err(e) = container.try_start_next_action(&ctx).await {
                                tracing::error!(
                                    "Failed to start next action after completion: {}",
                                    e
                                );
                            }
                        } else {
                            tracing::info!(
                                "Skipping cleanup script for workspace {} - no changes made by coding agent",
                                ctx.workspace.id
                            );

                            // Manually finalize task since we're bypassing normal execution flow
                            if !finalized {
                                container.finalize_task(&ctx).await;
                                finalized = true;
                            }
                        }
                    }

                    if let Err(e) = container.maybe_schedule_auto_retry(&ctx).await {
                        tracing::warn!("Auto retry scheduling failed: {}", e);
                    }

                    container.auto_retry_states.write().await.remove(&exec_id);

                    if container.should_finalize(&ctx) {
                        // Only execute queued messages if the execution succeeded
                        // If it failed or was killed, just clear the queue and finalize
                        let should_execute_queued = !matches!(
                            ctx.execution_process.status,
                            ExecutionProcessStatus::Failed | ExecutionProcessStatus::Killed
                        );

                        if let Some(queued_msg) =
                            container.queued_message_service.take_queued(ctx.session.id)
                        {
                            if should_execute_queued {
                                tracing::info!(
                                    "Found queued message for session {}, starting follow-up execution",
                                    ctx.session.id
                                );

                                // Delete the scratch since we're consuming the queued message
                                if let Err(e) = Scratch::delete(
                                    &db.pool,
                                    ctx.session.id,
                                    &ScratchType::DraftFollowUp,
                                )
                                .await
                                {
                                    tracing::warn!(
                                        "Failed to delete scratch after consuming queued message: {}",
                                        e
                                    );
                                }

                                // Execute the queued follow-up
                                if let Err(e) = container
                                    .start_queued_follow_up(&ctx, &queued_msg.data)
                                    .await
                                {
                                    tracing::error!("Failed to start queued follow-up: {}", e);
                                    // Fall back to finalization if follow-up fails
                                    if !finalized {
                                        container.finalize_task(&ctx).await;
                                    }
                                }
                            } else {
                                // Execution failed or was killed - discard the queued message and finalize
                                tracing::info!(
                                    "Discarding queued message for session {} due to execution status {:?}",
                                    ctx.session.id,
                                    ctx.execution_process.status
                                );
                                if !finalized {
                                    container.finalize_task(&ctx).await;
                                }
                            }
                        } else if !finalized {
                            container.finalize_task(&ctx).await;
                        }
                    }
                }

                container.end_finalization(exec_id).await;
            }

            // Now that commit/next-action/finalization steps for this process are complete,
            // capture the HEAD OID as the definitive "after" state (best-effort).
            container.update_after_head_commits(exec_id).await;

            // Cleanup msg store
            if let Some(msg_arc) = msg_stores.write().await.remove(&exec_id) {
                msg_arc.push_finished();
                tokio::time::sleep(Duration::from_millis(50)).await; // Wait for the finish message to propogate
                match Arc::try_unwrap(msg_arc) {
                    Ok(inner) => drop(inner),
                    Err(arc) => tracing::error!(
                        "There are still {} strong Arcs to MsgStore for {}",
                        Arc::strong_count(&arc),
                        exec_id
                    ),
                }
            }

            // Cleanup child handle
            child_store.write().await.remove(&exec_id);
        })
    }

    pub fn spawn_os_exit_watcher(
        &self,
        exec_id: Uuid,
    ) -> tokio::sync::oneshot::Receiver<std::io::Result<std::process::ExitStatus>> {
        let (tx, rx) = tokio::sync::oneshot::channel::<std::io::Result<std::process::ExitStatus>>();
        let child_store = self.child_store.clone();
        tokio::spawn(async move {
            loop {
                let child_lock = {
                    let map = child_store.read().await;
                    map.get(&exec_id).cloned()
                };
                if let Some(child_lock) = child_lock {
                    let mut child_handler = child_lock.write().await;
                    match child_handler.try_wait() {
                        Ok(Some(status)) => {
                            let _ = tx.send(Ok(status));
                            break;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            break;
                        }
                    }
                } else {
                    let _ = tx.send(Err(io::Error::other(format!(
                        "Child handle missing for {exec_id}"
                    ))));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });
        rx
    }

    pub fn dir_name_from_workspace(workspace_id: &Uuid, task_title: &str) -> String {
        let task_title_id = git_branch_id(task_title);
        format!("{}-{}", short_uuid(workspace_id), task_title_id)
    }

    async fn track_child_msgs_in_store(&self, id: Uuid, child: &mut AsyncGroupChild) {
        let store = Arc::new(MsgStore::new());

        let out = child.inner().stdout.take().expect("no stdout");
        let err = child.inner().stderr.take().expect("no stderr");

        // Map stdout bytes -> LogMsg::Stdout
        let out = ReaderStream::new(out)
            .map_ok(|chunk| LogMsg::Stdout(String::from_utf8_lossy(&chunk).into_owned()));

        // Map stderr bytes -> LogMsg::Stderr
        let err = ReaderStream::new(err)
            .map_ok(|chunk| LogMsg::Stderr(String::from_utf8_lossy(&chunk).into_owned()));

        // If you have a JSON Patch source, map it to LogMsg::JsonPatch too, then select all three.

        // Merge and forward into the store
        let merged = select(out, err); // Stream<Item = Result<LogMsg, io::Error>>
        store.clone().spawn_forwarder(merged);

        let mut map = self.msg_stores().write().await;
        map.insert(id, store);
    }

    /// Create a live diff log stream for ongoing attempts for WebSocket
    /// Returns a stream that owns the filesystem watcher - when dropped, watcher is cleaned up
    async fn create_live_diff_stream(
        &self,
        worktree_path: &Path,
        base_commit: &Commit,
        stats_only: bool,
        path_prefix: Option<String>,
    ) -> Result<DiffStreamHandle, ContainerError> {
        diff_stream::create(
            self.git().clone(),
            worktree_path.to_path_buf(),
            base_commit.clone(),
            stats_only,
            path_prefix,
        )
        .await
        .map_err(|e| ContainerError::Other(anyhow!("{e}")))
    }

    /// Extract the last assistant message from the MsgStore history
    fn extract_last_assistant_message(&self, exec_id: &Uuid) -> Option<String> {
        // Get the MsgStore for this execution
        let msg_stores = self.msg_stores.try_read().ok()?;
        let msg_store = msg_stores.get(exec_id)?;

        // Get the history and scan in reverse for the last assistant message
        let history = msg_store.get_history();

        for msg in history.iter().rev() {
            if let LogMsg::JsonPatch(patch) = msg {
                // Try to extract a NormalizedEntry from the patch
                if let Some((_, entry)) = extract_normalized_entry_from_patch(patch)
                    && matches!(entry.entry_type, NormalizedEntryType::AssistantMessage)
                {
                    let content = entry.content.trim();
                    if !content.is_empty() {
                        const MAX_SUMMARY_LENGTH: usize = 4096;
                        if content.len() > MAX_SUMMARY_LENGTH {
                            let truncated = truncate_to_char_boundary(content, MAX_SUMMARY_LENGTH);
                            return Some(format!("{truncated}..."));
                        }
                        return Some(content.to_string());
                    }
                }
            }
        }

        None
    }

    /// Update the coding agent turn summary with the final assistant message
    async fn update_executor_session_summary(&self, exec_id: &Uuid) -> Result<(), anyhow::Error> {
        // Check if there's a coding agent turn for this execution process
        let turn = CodingAgentTurn::find_by_execution_process_id(&self.db.pool, *exec_id).await?;

        if let Some(turn) = turn {
            // Only update if summary is not already set
            if turn.summary.is_none() {
                if let Some(summary) = self.extract_last_assistant_message(exec_id) {
                    CodingAgentTurn::update_summary(&self.db.pool, *exec_id, &summary).await?;
                } else {
                    tracing::debug!("No assistant message found for execution {}", exec_id);
                }
            }
        }

        Ok(())
    }

    /// Copy project files and images to the workspace.
    /// Skips files/images that already exist (fast no-op if all exist).
    async fn copy_files_and_images(
        &self,
        workspace_dir: &Path,
        workspace: &Workspace,
    ) -> Result<(), ContainerError> {
        let repos = WorkspaceRepo::find_repos_with_copy_files(&self.db.pool, workspace.id).await?;

        for repo in &repos {
            if let Some(copy_files) = &repo.copy_files
                && !copy_files.trim().is_empty()
            {
                let worktree_path = workspace_dir.join(&repo.name);
                self.copy_project_files(&repo.path, &worktree_path, copy_files)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(
                            "Failed to copy project files for repo '{}': {}",
                            repo.name,
                            e
                        );
                    });
            }
        }

        if let Err(e) = self
            .image_service
            .copy_images_by_task_to_worktree(workspace_dir, workspace.task_id)
            .await
        {
            tracing::warn!("Failed to copy task images to workspace: {}", e);
        }

        Ok(())
    }

    /// Create workspace-level CLAUDE.md and AGENTS.md files that import from each repo.
    /// Uses the @import syntax to reference each repo's config files.
    /// Skips creating files if they already exist or if no repos have the source file.
    async fn create_workspace_config_files(
        workspace_dir: &Path,
        repos: &[Repo],
    ) -> Result<(), ContainerError> {
        const CONFIG_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];

        for config_file in CONFIG_FILES {
            let workspace_config_path = workspace_dir.join(config_file);

            if workspace_config_path.exists() {
                tracing::debug!(
                    "Workspace config file {} already exists, skipping",
                    config_file
                );
                continue;
            }

            let mut import_lines = Vec::new();
            for repo in repos {
                let repo_config_path = workspace_dir.join(&repo.name).join(config_file);
                if repo_config_path.exists() {
                    import_lines.push(format!("@{}/{}", repo.name, config_file));
                }
            }

            if import_lines.is_empty() {
                tracing::debug!(
                    "No repos have {}, skipping workspace config creation",
                    config_file
                );
                continue;
            }

            let content = import_lines.join("\n") + "\n";
            if let Err(e) = tokio::fs::write(&workspace_config_path, &content).await {
                tracing::warn!(
                    "Failed to create workspace config file {}: {}",
                    config_file,
                    e
                );
                continue;
            }

            tracing::info!(
                "Created workspace {} with {} import(s)",
                config_file,
                import_lines.len()
            );
        }

        Ok(())
    }

    /// Start a follow-up execution from a queued message
    async fn start_queued_follow_up(
        &self,
        ctx: &ExecutionContext,
        queued_data: &DraftFollowUpData,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Get executor profile from the latest CodingAgent process in this session
        let initial_executor_profile_id =
            ExecutionProcess::latest_executor_profile_for_session(&self.db.pool, ctx.session.id)
                .await
                .map_err(|e| {
                    ContainerError::Other(anyhow!("Failed to get executor profile: {e}"))
                })?;

        let executor_profile_id = ExecutorProfileId {
            executor: initial_executor_profile_id.executor,
            variant: queued_data.variant.clone(),
        };

        // Get latest agent session ID for session continuity (from coding agent turns)
        let latest_agent_session_id = ExecutionProcess::find_latest_coding_agent_turn_session_id(
            &self.db.pool,
            ctx.session.id,
        )
        .await?;

        let project_repos =
            ProjectRepo::find_by_project_id_with_names(&self.db.pool, ctx.project.id).await?;
        let cleanup_action = self.cleanup_actions_for_repos(&project_repos);

        let working_dir = ctx
            .workspace
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();
        let image_paths = match self
            .image_service
            .image_path_map_for_task(ctx.task.id)
            .await
        {
            Ok(map) if !map.is_empty() => Some(map),
            Ok(_) => None,
            Err(err) => {
                tracing::warn!("Failed to resolve task image paths: {}", err);
                None
            }
        };

        let action_type = if let Some(agent_session_id) = latest_agent_session_id {
            ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                prompt: queued_data.message.clone(),
                session_id: agent_session_id,
                executor_profile_id: executor_profile_id.clone(),
                working_dir: working_dir.clone(),
                image_paths: image_paths.clone(),
            })
        } else {
            ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                prompt: queued_data.message.clone(),
                executor_profile_id: executor_profile_id.clone(),
                working_dir,
                image_paths,
            })
        };

        let action = ExecutorAction::new(action_type, cleanup_action.map(Box::new));

        self.start_execution(
            &ctx.workspace,
            &ctx.session,
            &action,
            &ExecutionProcessRunReason::CodingAgent,
        )
        .await
    }
}

fn failure_exit_status() -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatusExt::from_raw(256) // Exit code 1 (shifted by 8 bits)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatusExt::from_raw(1)
    }
}

#[async_trait]
impl ContainerService for LocalContainerService {
    fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>> {
        &self.msg_stores
    }

    fn db(&self) -> &DBService {
        &self.db
    }

    fn git(&self) -> &GitService {
        &self.git
    }

    fn image_service(&self) -> &ImageService {
        &self.image_service
    }

    fn notification_service(&self) -> &NotificationService {
        &self.notification_service
    }

    async fn git_branch_prefix(&self) -> String {
        self.config.read().await.git_branch_prefix.clone()
    }

    fn workspace_to_current_dir(&self, workspace: &Workspace) -> PathBuf {
        PathBuf::from(workspace.container_ref.clone().unwrap_or_default())
    }

    async fn create(&self, workspace: &Workspace) -> Result<ContainerRef, ContainerError> {
        let task = workspace
            .parent_task(&self.db.pool)
            .await?
            .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

        let workspace_dir_name =
            LocalContainerService::dir_name_from_workspace(&workspace.id, &task.title);
        let workspace_dir = WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name);

        let workspace_repos =
            WorkspaceRepo::find_by_workspace_id(&self.db.pool, workspace.id).await?;
        if workspace_repos.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        let target_branches: HashMap<_, _> = workspace_repos
            .iter()
            .map(|wr| (wr.repo_id, wr.target_branch.clone()))
            .collect();

        let workspace_inputs: Vec<RepoWorkspaceInput> = repositories
            .iter()
            .map(|repo| {
                let target_branch = target_branches.get(&repo.id).cloned().unwrap_or_default();
                RepoWorkspaceInput::new(repo.clone(), target_branch)
            })
            .collect();

        let created_workspace = WorkspaceManager::create_workspace(
            &workspace_dir,
            &workspace_inputs,
            &workspace.branch,
        )
        .await?;

        // Copy project files and images to workspace
        self.copy_files_and_images(&created_workspace.workspace_dir, workspace)
            .await?;

        Self::create_workspace_config_files(&created_workspace.workspace_dir, &repositories)
            .await?;

        Workspace::update_container_ref(
            &self.db.pool,
            workspace.id,
            &created_workspace.workspace_dir.to_string_lossy(),
        )
        .await?;

        Ok(created_workspace
            .workspace_dir
            .to_string_lossy()
            .to_string())
    }

    async fn delete(&self, workspace: &Workspace) -> Result<(), ContainerError> {
        self.try_stop(workspace, true).await;
        Self::cleanup_workspace(&self.db, workspace).await;
        Ok(())
    }

    async fn ensure_container_exists(
        &self,
        workspace: &Workspace,
    ) -> Result<ContainerRef, ContainerError> {
        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        if repositories.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let workspace_dir = if let Some(container_ref) = &workspace.container_ref {
            PathBuf::from(container_ref)
        } else {
            let task = workspace
                .parent_task(&self.db.pool)
                .await?
                .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
            let workspace_dir_name =
                LocalContainerService::dir_name_from_workspace(&workspace.id, &task.title);
            WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name)
        };

        WorkspaceManager::ensure_workspace_exists(&workspace_dir, &repositories, &workspace.branch)
            .await?;

        if workspace.container_ref.is_none() {
            Workspace::update_container_ref(
                &self.db.pool,
                workspace.id,
                &workspace_dir.to_string_lossy(),
            )
            .await?;
        }

        // Copy project files and images (fast no-op if already exist)
        self.copy_files_and_images(&workspace_dir, workspace)
            .await?;

        Self::create_workspace_config_files(&workspace_dir, &repositories).await?;

        Ok(workspace_dir.to_string_lossy().to_string())
    }

    async fn is_container_clean(&self, workspace: &Workspace) -> Result<bool, ContainerError> {
        let Some(container_ref) = &workspace.container_ref else {
            return Ok(true);
        };

        let workspace_dir = PathBuf::from(container_ref);
        if !workspace_dir.exists() {
            return Ok(true);
        }

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        for repo in &repositories {
            let worktree_path = workspace_dir.join(&repo.name);
            if worktree_path.exists() && !self.git().is_worktree_clean(&worktree_path)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn start_execution_inner(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError> {
        // Get the worktree path
        let container_ref = workspace
            .container_ref
            .as_ref()
            .ok_or(ContainerError::Other(anyhow!(
                "Container ref not found for workspace"
            )))?;
        let current_dir = PathBuf::from(container_ref);

        let approvals_service: Arc<dyn ExecutorApprovalService> =
            match executor_action.base_executor() {
                Some(
                    BaseCodingAgent::Codex
                    | BaseCodingAgent::ClaudeCode
                    | BaseCodingAgent::Gemini
                    | BaseCodingAgent::QwenCode
                    | BaseCodingAgent::Opencode,
                ) => ExecutorApprovalBridge::new(
                    self.approvals.clone(),
                    self.db.clone(),
                    self.notification_service.clone(),
                    execution_process.id,
                ),
                _ => Arc::new(NoopExecutorApprovalService {}),
            };

        // Build ExecutionEnv with VK_* variables
        let mut env = ExecutionEnv::new();

        // Load task and project context for environment variables
        let task = workspace
            .parent_task(&self.db.pool)
            .await?
            .ok_or(ContainerError::Other(anyhow!(
                "Task not found for workspace"
            )))?;
        let project = task
            .parent_project(&self.db.pool)
            .await?
            .ok_or(ContainerError::Other(anyhow!("Project not found for task")))?;

        env.insert("VK_PROJECT_NAME", &project.name);
        env.insert("VK_PROJECT_ID", project.id.to_string());
        env.insert("VK_TASK_ID", task.id.to_string());
        env.insert("VK_WORKSPACE_ID", workspace.id.to_string());
        env.insert("VK_WORKSPACE_BRANCH", &workspace.branch);

        // Create the child and stream, add to execution tracker with timeout
        let mut spawned = tokio::time::timeout(
            Duration::from_secs(30),
            executor_action.spawn(&current_dir, approvals_service, &env),
        )
        .await
        .map_err(|_| {
            ContainerError::Other(anyhow!(
                "Timeout: process took more than 30 seconds to start"
            ))
        })??;

        self.track_child_msgs_in_store(execution_process.id, &mut spawned.child)
            .await;

        self.add_child_to_store(execution_process.id, spawned.child)
            .await;

        // Store interrupt sender for graceful shutdown
        if let Some(interrupt_sender) = spawned.interrupt_sender {
            self.add_interrupt_sender(execution_process.id, interrupt_sender)
                .await;
        }

        // Spawn unified exit monitor: watches OS exit and optional executor signal
        let _hn = self.spawn_exit_monitor(&execution_process.id, spawned.exit_signal);

        Ok(())
    }

    async fn stop_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError> {
        if !self.begin_finalization(execution_process.id).await {
            tracing::debug!(
                "Skipping stop_execution for {} because another finalizer is active",
                execution_process.id
            );
            return Ok(());
        }

        let result = async {
            let child = self
                .get_child_from_store(&execution_process.id)
                .await
                .ok_or_else(|| {
                    ContainerError::Other(anyhow!("Child process not found for execution"))
                })?;
            let exit_code = if status == ExecutionProcessStatus::Completed {
                Some(0)
            } else {
                None
            };

            ExecutionProcess::update_completion(
                &self.db.pool,
                execution_process.id,
                status,
                exit_code,
            )
            .await?;

            // Try graceful interrupt first, then force kill
            if let Some(interrupt_sender) = self.take_interrupt_sender(&execution_process.id).await
            {
                // Send interrupt signal (ignore error if receiver dropped)
                let _ = interrupt_sender.send(());

                // Wait for graceful exit with timeout
                let graceful_exit = {
                    let mut child_guard = child.write().await;
                    tokio::time::timeout(Duration::from_secs(5), child_guard.wait()).await
                };

                match graceful_exit {
                    Ok(Ok(_)) => {
                        tracing::debug!(
                            "Process {} exited gracefully after interrupt",
                            execution_process.id
                        );
                    }
                    Ok(Err(e)) => {
                        tracing::info!("Error waiting for process {}: {}", execution_process.id, e);
                    }
                    Err(_) => {
                        tracing::debug!(
                            "Graceful shutdown timed out for process {}, force killing",
                            execution_process.id
                        );
                    }
                }
            }

            // Kill the child process and remove from the store
            {
                let mut child_guard = child.write().await;
                if let Err(e) = command::kill_process_group(&mut child_guard).await {
                    tracing::error!(
                        "Failed to stop execution process {}: {}",
                        execution_process.id,
                        e
                    );
                    return Err(e);
                }
            }
            self.remove_child_from_store(&execution_process.id).await;

            // Mark the process finished in the MsgStore
            if let Some(msg) = self.msg_stores.write().await.remove(&execution_process.id) {
                msg.push_finished();
            }

            // Update task status to InReview when execution is stopped
            if let Ok(ctx) =
                ExecutionProcess::load_context(&self.db.pool, execution_process.id).await
                && !matches!(
                    ctx.execution_process.run_reason,
                    ExecutionProcessRunReason::DevServer
                )
            {
                match Task::update_status(&self.db.pool, ctx.task.id, TaskStatus::InReview).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Failed to update task status to InReview: {e}");
                    }
                }
            }

            tracing::debug!(
                "Execution process {} stopped successfully",
                execution_process.id
            );

            // Record after-head commit OID (best-effort)
            self.update_after_head_commits(execution_process.id).await;

            Ok(())
        }
        .await;

        self.end_finalization(execution_process.id).await;
        result
    }

    async fn stop_execution_force(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError> {
        if self
            .get_child_from_store(&execution_process.id)
            .await
            .is_some()
        {
            return self.stop_execution(execution_process, status).await;
        }

        if !self.begin_finalization(execution_process.id).await {
            tracing::debug!(
                "Skipping stop_execution_force for {} because another finalizer is active",
                execution_process.id
            );
            return Ok(());
        }

        let result = async {
            let exit_code = if status == ExecutionProcessStatus::Completed {
                Some(0)
            } else {
                None
            };

            ExecutionProcess::update_completion(
                &self.db.pool,
                execution_process.id,
                status,
                exit_code,
            )
            .await?;

            let _ = self.take_interrupt_sender(&execution_process.id).await;
            self.remove_child_from_store(&execution_process.id).await;

            if let Some(msg) = self.msg_stores.write().await.remove(&execution_process.id) {
                msg.push_finished();
            }

            if let Ok(ctx) =
                ExecutionProcess::load_context(&self.db.pool, execution_process.id).await
                && !matches!(
                    ctx.execution_process.run_reason,
                    ExecutionProcessRunReason::DevServer
                )
            {
                match Task::update_status(&self.db.pool, ctx.task.id, TaskStatus::InReview).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Failed to update task status to InReview: {e}");
                    }
                }
            }

            tracing::debug!(
                "Execution process {} force-stopped without a child handle",
                execution_process.id
            );

            self.update_after_head_commits(execution_process.id).await;

            Ok(())
        }
        .await;

        self.end_finalization(execution_process.id).await;
        result
    }

    async fn stream_diff(
        &self,
        workspace: &Workspace,
        options: DiffStreamOptions,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError>
    {
        let stats_only = options.stats_only;
        let force = options.force;
        let guard_preset = self.config.read().await.diff_preview_guard.clone();
        let workspace_repos =
            WorkspaceRepo::find_by_workspace_id(&self.db.pool, workspace.id).await?;
        let target_branches: HashMap<_, _> = workspace_repos
            .iter()
            .map(|wr| (wr.repo_id, wr.target_branch.clone()))
            .collect();

        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, workspace.id).await?;

        let build_summary_stream =
            |summary: DiffSummary, blocked: bool, blocked_reason: Option<&str>| {
                let patch = serde_json::from_value(json!([
                    { "op": "add", "path": "/summary", "value": summary },
                    { "op": "add", "path": "/blocked", "value": blocked },
                    { "op": "add", "path": "/blockedReason", "value": blocked_reason },
                ]))
                .expect("diff summary patch");
                futures::stream::iter(vec![Ok(LogMsg::JsonPatch(patch)), Ok(LogMsg::Finished)])
            };

        let workspace_root = match workspace
            .container_ref
            .as_ref()
            .map(PathBuf::from)
            .filter(|path| path.exists())
        {
            Some(path) => path,
            None => match self.ensure_container_exists(workspace).await {
                Ok(container_ref) => PathBuf::from(container_ref),
                Err(err) => {
                    tracing::warn!(
                        "Failed to ensure workspace container for diff stream {}: {}",
                        workspace.id,
                        err
                    );
                    let stream =
                        build_summary_stream(DiffSummary::default(), true, Some("summary_failed"));
                    return Ok(Box::pin(stream));
                }
            },
        };

        let mut repo_inputs = Vec::new();
        let mut skipped_repos = 0usize;
        let mut total_repos = 0usize;
        for repo in repositories {
            total_repos += 1;
            let worktree_path = workspace_root.join(&repo.name);
            let branch = &workspace.branch;

            let Some(target_branch) = target_branches.get(&repo.id) else {
                tracing::warn!(
                    "Skipping diff stream for repo {}: no target branch configured",
                    repo.name
                );
                skipped_repos += 1;
                continue;
            };

            let base_commit = match self
                .git()
                .get_base_commit(&repo.path, branch, target_branch)
            {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "Skipping diff stream for repo {}: failed to get base commit: {}",
                        repo.name,
                        e
                    );
                    skipped_repos += 1;
                    continue;
                }
            };

            repo_inputs.push((repo, worktree_path, base_commit));
        }

        if repo_inputs.is_empty() {
            let blocked_reason = if total_repos > 0 && skipped_repos == total_repos {
                Some("summary_failed")
            } else {
                None
            };
            let blocked = blocked_reason.is_some();
            let stream = build_summary_stream(DiffSummary::default(), blocked, blocked_reason);
            return Ok(Box::pin(stream));
        }

        let mut summary = DiffSummary::default();
        let mut summary_failed = false;
        for (repo, worktree_path, base_commit) in &repo_inputs {
            match self
                .git
                .get_worktree_diff_summary(worktree_path, base_commit, None)
            {
                Ok(repo_summary) => {
                    summary.file_count = summary.file_count.saturating_add(repo_summary.file_count);
                    summary.added = summary.added.saturating_add(repo_summary.added);
                    summary.deleted = summary.deleted.saturating_add(repo_summary.deleted);
                    summary.total_bytes =
                        summary.total_bytes.saturating_add(repo_summary.total_bytes);
                }
                Err(e) => {
                    summary_failed = true;
                    tracing::warn!(
                        "Failed to compute diff summary for repo {}: {}",
                        repo.name,
                        e
                    );
                }
            }
        }

        let guard_enabled =
            diff_stream::diff_preview_guard_thresholds(guard_preset.clone()).is_some();
        let blocked = !force
            && guard_enabled
            && (summary_failed || diff_stream::diff_preview_guard_exceeded(&summary, guard_preset));
        let blocked_reason = if blocked {
            if summary_failed {
                Some("summary_failed")
            } else {
                Some("threshold_exceeded")
            }
        } else {
            None
        };

        let summary_patch = {
            let patch = serde_json::from_value(json!([
                { "op": "add", "path": "/summary", "value": summary },
                { "op": "add", "path": "/blocked", "value": blocked },
                { "op": "add", "path": "/blockedReason", "value": blocked_reason },
            ]))
            .expect("diff summary patch");
            LogMsg::JsonPatch(patch)
        };

        if stats_only || blocked {
            let stream = futures::stream::iter(vec![Ok(summary_patch), Ok(LogMsg::Finished)]);
            return Ok(Box::pin(stream));
        }

        let mut streams = Vec::new();
        for (repo, worktree_path, base_commit) in repo_inputs {
            let stream = self
                .create_live_diff_stream(
                    &worktree_path,
                    &base_commit,
                    false,
                    Some(repo.name.clone()),
                )
                .await?;
            streams.push(Box::pin(stream));
        }

        if streams.is_empty() {
            return Ok(Box::pin(futures::stream::empty()));
        }

        let summary_stream = futures::stream::iter(vec![Ok(summary_patch)]);
        let merged_stream = futures::stream::select_all(streams);
        Ok(Box::pin(summary_stream.chain(merged_stream)))
    }

    async fn try_commit_changes(&self, ctx: &ExecutionContext) -> Result<bool, ContainerError> {
        if !matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::CodingAgent | ExecutionProcessRunReason::CleanupScript,
        ) {
            return Ok(false);
        }

        let message = self.get_commit_message(ctx).await;

        let container_ref = ctx
            .workspace
            .container_ref
            .as_ref()
            .ok_or_else(|| ContainerError::Other(anyhow!("Container reference not found")))?;
        let workspace_root = PathBuf::from(container_ref);

        let repos_with_changes = self.check_repos_for_changes(&workspace_root, &ctx.repos)?;
        if repos_with_changes.is_empty() {
            tracing::debug!("No changes to commit in any repository");
            return Ok(false);
        }

        Ok(self.commit_repos(repos_with_changes, &message).await)
    }

    /// Copy files from the original project directory to the worktree.
    /// Skips files that already exist at target with same size.
    async fn copy_project_files(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        copy_files: &str,
    ) -> Result<(), ContainerError> {
        let source_dir = source_dir.to_path_buf();
        let target_dir = target_dir.to_path_buf();
        let copy_files = copy_files.to_string();

        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || {
                copy::copy_project_files_impl(&source_dir, &target_dir, &copy_files)
            }),
        )
        .await
        .map_err(|_| ContainerError::Other(anyhow!("Copy project files timed out after 30s")))?
        .map_err(|e| ContainerError::Other(anyhow!("Copy files task failed: {e}")))?
    }

    async fn kill_all_running_processes(&self) -> Result<(), ContainerError> {
        tracing::info!("Killing all running processes");
        let running_processes = ExecutionProcess::find_running(&self.db.pool).await?;

        for process in running_processes {
            if let Err(error) = self
                .stop_execution(&process, ExecutionProcessStatus::Killed)
                .await
            {
                tracing::error!(
                    "Failed to cleanly kill running execution process {:?}: {:?}",
                    process,
                    error
                );
            }
        }

        Ok(())
    }
}
fn success_exit_status() -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatusExt::from_raw(0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatusExt::from_raw(0)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::Barrier;

    use super::*;

    #[tokio::test]
    async fn finalization_tracker_allows_single_owner_during_race() {
        let tracker = FinalizationTracker::default();
        let execution_process_id = Uuid::new_v4();
        let barrier = Arc::new(Barrier::new(3));

        let tracker_a = tracker.clone();
        let barrier_a = barrier.clone();
        let task_a = tokio::spawn(async move {
            barrier_a.wait().await;
            tracker_a.begin(execution_process_id).await
        });

        let tracker_b = tracker.clone();
        let barrier_b = barrier.clone();
        let task_b = tokio::spawn(async move {
            barrier_b.wait().await;
            tracker_b.begin(execution_process_id).await
        });

        barrier.wait().await;
        let owns_a = task_a.await.unwrap();
        let owns_b = task_b.await.unwrap();

        assert_ne!(owns_a, owns_b);
    }

    #[tokio::test]
    async fn finalization_tracker_reacquires_after_release() {
        let tracker = FinalizationTracker::default();
        let execution_process_id = Uuid::new_v4();

        assert!(tracker.begin(execution_process_id).await);
        tracker.end(execution_process_id).await;
        assert!(tracker.begin(execution_process_id).await);
    }
}
