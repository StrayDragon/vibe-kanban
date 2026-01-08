use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use anyhow::{Error as AnyhowError, anyhow};
use async_trait::async_trait;
use dashmap::DashSet;
use db::{
    DBService,
    models::{
        coding_agent_turn::{CodingAgentTurn, CreateCodingAgentTurn},
        execution_process::{
            CreateExecutionProcess, ExecutionContext, ExecutionProcess, ExecutionProcessRunReason,
            ExecutionProcessStatus,
        },
        execution_process_log_entries::{ExecutionProcessLogEntry, LogEntryRow},
        execution_process_logs::ExecutionProcessLogs,
        execution_process_repo_state::{
            CreateExecutionProcessRepoState, ExecutionProcessRepoState,
        },
        project::{Project, UpdateProject},
        project_repo::{ProjectRepo, ProjectRepoWithName},
        repo::Repo,
        session::{CreateSession, Session, SessionError},
        task::{Task, TaskStatus},
        workspace::{Workspace, WorkspaceError},
        workspace_repo::WorkspaceRepo,
    },
};
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        coding_agent_initial::CodingAgentInitialRequest,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    executors::{ExecutorError, StandardCodingAgentExecutor},
    logs::{NormalizedEntry, NormalizedEntryError, NormalizedEntryType, utils::ConversationPatch},
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use futures::{StreamExt, future};
use once_cell::sync::Lazy;
use sqlx::Error as SqlxError;
use thiserror::Error;
use tokio::{sync::RwLock, task::JoinHandle};
use utils::{
    log_entries::LogEntryChannel,
    log_msg::LogMsg,
    msg_store::{LogEntryEvent, LogEntrySnapshot, MsgStore},
    text::{git_branch_id, short_uuid},
};
use uuid::Uuid;

use crate::services::{
    git::{GitService, GitServiceError},
    notification::NotificationService,
    workspace_manager::WorkspaceError as WorkspaceManagerError,
    worktree_manager::WorktreeError,
};
pub type ContainerRef = String;

static LOG_ENTRY_BACKFILL_COMPLETE: Lazy<DashSet<String>> = Lazy::new(DashSet::new);

#[derive(Debug, Clone, Copy, Default)]
pub struct DiffStreamOptions {
    pub stats_only: bool,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct LogHistoryPageData {
    pub entries: Vec<LogEntrySnapshot>,
    pub has_more: bool,
}

#[derive(Debug, Error)]
pub enum ContainerError {
    #[error(transparent)]
    GitServiceError(#[from] GitServiceError),
    #[error(transparent)]
    Sqlx(#[from] SqlxError),
    #[error(transparent)]
    ExecutorError(#[from] ExecutorError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    WorkspaceManager(#[from] WorkspaceManagerError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to kill process: {0}")]
    KillFailed(std::io::Error),
    #[error(transparent)]
    Other(#[from] AnyhowError), // Catches any unclassified errors
}

#[async_trait]
pub trait ContainerService {
    fn msg_stores(&self) -> &Arc<RwLock<HashMap<Uuid, Arc<MsgStore>>>>;

    fn db(&self) -> &DBService;

    fn git(&self) -> &GitService;

    fn notification_service(&self) -> &NotificationService;

    fn workspace_to_current_dir(&self, workspace: &Workspace) -> PathBuf;

    async fn create(&self, workspace: &Workspace) -> Result<ContainerRef, ContainerError>;

    async fn kill_all_running_processes(&self) -> Result<(), ContainerError>;

    async fn delete(&self, workspace: &Workspace) -> Result<(), ContainerError>;

    /// Check if a task has any running execution processes
    async fn has_running_processes(&self, task_id: Uuid) -> Result<bool, ContainerError> {
        let workspaces = Workspace::fetch_all(&self.db().pool, Some(task_id)).await?;

        for workspace in workspaces {
            let sessions = Session::find_by_workspace_id(&self.db().pool, workspace.id).await?;
            for session in sessions {
                if let Ok(processes) =
                    ExecutionProcess::find_by_session_id(&self.db().pool, session.id, false).await
                {
                    for process in processes {
                        if process.status == ExecutionProcessStatus::Running {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// A context is finalized when
    /// - Always when the execution process has failed or been killed
    /// - Never when the run reason is DevServer
    /// - Never when a setup script has no next_action (parallel mode)
    /// - The next action is None (no follow-up actions)
    fn should_finalize(&self, ctx: &ExecutionContext) -> bool {
        // Never finalize DevServer processes
        if matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::DevServer
        ) {
            return false;
        }

        // Never finalize setup scripts without a next_action (parallel mode).
        // In sequential mode, setup scripts have next_action pointing to coding agent,
        // so they won't finalize anyway (handled by next_action.is_none() check below).
        let action = ctx.execution_process.executor_action().unwrap();
        if matches!(
            ctx.execution_process.run_reason,
            ExecutionProcessRunReason::SetupScript
        ) && action.next_action.is_none()
        {
            return false;
        }

        // Always finalize failed or killed executions, regardless of next action
        if matches!(
            ctx.execution_process.status,
            ExecutionProcessStatus::Failed | ExecutionProcessStatus::Killed
        ) {
            return true;
        }

        // Otherwise, finalize only if no next action
        action.next_action.is_none()
    }

    /// Finalize task execution by updating status to InReview and sending notifications
    async fn finalize_task(&self, ctx: &ExecutionContext) {
        match Task::update_status(&self.db().pool, ctx.task.id, TaskStatus::InReview).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Failed to update task status to InReview: {e}");
            }
        }

        // Skip notification if process was intentionally killed by user
        if matches!(ctx.execution_process.status, ExecutionProcessStatus::Killed) {
            return;
        }

        let title = format!("Task Complete: {}", ctx.task.title);
        let message = match ctx.execution_process.status {
            ExecutionProcessStatus::Completed => format!(
                "✅ '{}' completed successfully\nBranch: {:?}\nExecutor: {:?}",
                ctx.task.title, ctx.workspace.branch, ctx.session.executor
            ),
            ExecutionProcessStatus::Failed => format!(
                "❌ '{}' execution failed\nBranch: {:?}\nExecutor: {:?}",
                ctx.task.title, ctx.workspace.branch, ctx.session.executor
            ),
            _ => {
                tracing::warn!(
                    "Tried to notify workspace completion for {} but process is still running!",
                    ctx.workspace.id
                );
                return;
            }
        };
        self.notification_service().notify(&title, &message).await;
    }

    /// Cleanup executions marked as running in the db, call at startup
    async fn cleanup_orphan_executions(&self) -> Result<(), ContainerError> {
        let running_processes = ExecutionProcess::find_running(&self.db().pool).await?;
        for process in running_processes {
            tracing::info!(
                "Found orphaned execution process {} for session {}",
                process.id,
                process.session_id
            );
            // Update the execution process status first
            if let Err(e) = ExecutionProcess::update_completion(
                &self.db().pool,
                process.id,
                ExecutionProcessStatus::Failed,
                None, // No exit code for orphaned processes
            )
            .await
            {
                tracing::error!(
                    "Failed to update orphaned execution process {} status: {}",
                    process.id,
                    e
                );
                continue;
            }
            // Capture after-head commit OID per repository
            if let Ok(ctx) = ExecutionProcess::load_context(&self.db().pool, process.id).await
                && let Some(ref container_ref) = ctx.workspace.container_ref
            {
                let workspace_root = PathBuf::from(container_ref);
                for repo in &ctx.repos {
                    let repo_path = workspace_root.join(&repo.name);
                    if let Ok(head) = self.git().get_head_info(&repo_path)
                        && let Err(err) = ExecutionProcessRepoState::update_after_head_commit(
                            &self.db().pool,
                            process.id,
                            repo.id,
                            &head.oid,
                        )
                        .await
                    {
                        tracing::warn!(
                            "Failed to update after_head_commit for repo {} on process {}: {}",
                            repo.id,
                            process.id,
                            err
                        );
                    }
                }
            }
            // Process marked as failed
            tracing::info!("Marked orphaned execution process {} as failed", process.id);
            // Update task status to InReview for coding agent and setup script failures
            if matches!(
                process.run_reason,
                ExecutionProcessRunReason::CodingAgent
                    | ExecutionProcessRunReason::SetupScript
                    | ExecutionProcessRunReason::CleanupScript
            ) && let Ok(Some(session)) =
                Session::find_by_id(&self.db().pool, process.session_id).await
                && let Ok(Some(workspace)) =
                    Workspace::find_by_id(&self.db().pool, session.workspace_id).await
                && let Ok(Some(task)) = workspace.parent_task(&self.db().pool).await
            {
                match Task::update_status(&self.db().pool, task.id, TaskStatus::InReview).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(
                            "Failed to update task status to InReview for orphaned session: {}",
                            e
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Backfill before_head_commit for legacy execution processes.
    /// Rules:
    /// - If a process has after_head_commit and missing before_head_commit,
    ///   then set before_head_commit to the previous process's after_head_commit.
    /// - If there is no previous process, set before_head_commit to the base branch commit.
    async fn backfill_before_head_commits(&self) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let rows = ExecutionProcess::list_missing_before_context(pool).await?;
        for row in rows {
            // Skip if no after commit at all (shouldn't happen due to WHERE)
            // Prefer previous process after-commit if present
            let mut before = row.prev_after_head_commit.clone();

            // Fallback to base branch commit OID
            if before.is_none() {
                let repo_path = std::path::Path::new(row.repo_path.as_deref().unwrap_or_default());
                match self
                    .git()
                    .get_branch_oid(repo_path, row.target_branch.as_str())
                {
                    Ok(oid) => before = Some(oid),
                    Err(e) => {
                        tracing::warn!(
                            "Backfill: Failed to resolve base branch OID for workspace {} (branch {}): {}",
                            row.workspace_id,
                            row.target_branch,
                            e
                        );
                    }
                }
            }

            if let Some(before_oid) = before
                && let Err(e) = ExecutionProcessRepoState::update_before_head_commit(
                    pool,
                    row.id,
                    row.repo_id,
                    &before_oid,
                )
                .await
            {
                tracing::warn!(
                    "Backfill: Failed to update before_head_commit for process {}: {}",
                    row.id,
                    e
                );
            }
        }

        Ok(())
    }

    /// Backfill repo names that were migrated with a sentinel placeholder.
    /// Also backfills dev_script_working_dir and agent_working_dir for single-repo projects.
    async fn backfill_repo_names(&self) -> Result<(), ContainerError> {
        let pool = &self.db().pool;
        let repos = Repo::list_needing_name_fix(pool).await?;

        if repos.is_empty() {
            return Ok(());
        }

        tracing::info!("Backfilling {} repo names", repos.len());

        for repo in repos {
            let name = repo
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&repo.id.to_string())
                .to_string();

            Repo::update_name(pool, repo.id, &name, &name).await?;

            // Also update dev_script_working_dir and agent_working_dir for single-repo projects
            let project_repos = ProjectRepo::find_by_repo_id(pool, repo.id).await?;
            for pr in project_repos {
                let all_repos = ProjectRepo::find_by_project_id(pool, pr.project_id).await?;
                if all_repos.len() == 1
                    && let Some(project) = Project::find_by_id(pool, pr.project_id).await?
                {
                    let needs_dev_script_working_dir = project
                        .dev_script
                        .as_ref()
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
                        && project
                            .dev_script_working_dir
                            .as_ref()
                            .map(|s| s.is_empty())
                            .unwrap_or(true);

                    let needs_default_agent_working_dir = project
                        .default_agent_working_dir
                        .as_ref()
                        .map(|s| s.is_empty())
                        .unwrap_or(true);

                    if needs_dev_script_working_dir || needs_default_agent_working_dir {
                        Project::update(
                            pool,
                            pr.project_id,
                            &UpdateProject {
                                name: Some(project.name.clone()),
                                dev_script: project.dev_script.clone(),
                                dev_script_working_dir: if needs_dev_script_working_dir {
                                    Some(name.clone())
                                } else {
                                    project.dev_script_working_dir.clone()
                                },
                                default_agent_working_dir: if needs_default_agent_working_dir {
                                    Some(name.clone())
                                } else {
                                    project.default_agent_working_dir.clone()
                                },
                            },
                        )
                        .await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Backfill execution log entries at startup (blocking, console logs only).
    async fn backfill_log_entries_startup(&self) -> Result<(), ContainerError> {
        const LOG_EVERY_PROCESSES: usize = 25;
        const LOG_EVERY_BYTES: i64 = 100 * 1024 * 1024;

        let summaries =
            ExecutionProcessLogs::list_execution_ids_with_bytes(&self.db().pool).await?;
        if summaries.is_empty() {
            return Ok(());
        }

        let total_bytes: i64 = summaries.iter().map(|s| s.total_bytes).sum();
        tracing::info!(
            "log-history backfill starting: processes={}, total_bytes={}, mode=startup",
            summaries.len(),
            total_bytes
        );

        let start = Instant::now();
        let mut processed = 0usize;
        let mut entries = 0usize;
        let mut bytes = 0i64;
        let mut next_bytes_report = LOG_EVERY_BYTES;

        for summary in summaries {
            processed += 1;
            bytes = bytes.saturating_add(summary.total_bytes);

            match self
                .backfill_log_entries_for_execution(summary.execution_id)
                .await
            {
                Ok(count) => entries = entries.saturating_add(count),
                Err(err) => {
                    tracing::warn!(
                        "log-history backfill error: execution_id={}, error={}",
                        summary.execution_id,
                        err
                    );
                }
            }

            if processed % LOG_EVERY_PROCESSES == 0 || bytes >= next_bytes_report {
                tracing::info!(
                    "log-history backfill progress: processed={}, entries={}, bytes={}, elapsed_ms={}",
                    processed,
                    entries,
                    bytes,
                    start.elapsed().as_millis()
                );
                while bytes >= next_bytes_report {
                    next_bytes_report = next_bytes_report.saturating_add(LOG_EVERY_BYTES);
                }
            }
        }

        tracing::info!(
            "log-history backfill complete: processes={}, entries={}, bytes={}, elapsed_ms={}",
            processed,
            entries,
            bytes,
            start.elapsed().as_millis()
        );

        Ok(())
    }

    async fn backfill_log_entries_for_execution(
        &self,
        execution_id: Uuid,
    ) -> Result<usize, ContainerError> {
        let mut total = 0usize;
        total = total.saturating_add(
            self.backfill_log_entries_if_incomplete(execution_id, LogEntryChannel::Raw)
                .await?,
        );
        total = total.saturating_add(
            self.backfill_log_entries_if_incomplete(execution_id, LogEntryChannel::Normalized)
                .await?,
        );
        Ok(total)
    }

    async fn backfill_log_entries_if_incomplete(
        &self,
        execution_id: Uuid,
        channel: LogEntryChannel,
    ) -> Result<usize, ContainerError> {
        let cache_key = format!("{execution_id}:{channel}");
        if LOG_ENTRY_BACKFILL_COMPLETE.contains(&cache_key) {
            return Ok(0);
        }

        let existing = ExecutionProcessLogEntry::stats(&self.db().pool, execution_id, channel)
            .await?;

        let entries = match channel {
            LogEntryChannel::Raw => self.collect_raw_entries_from_jsonl(execution_id).await?,
            LogEntryChannel::Normalized => {
                self.collect_normalized_entries_from_jsonl(execution_id).await?
            }
        };

        let Some((expected_count, expected_min, expected_max)) = entry_stats(&entries) else {
            LOG_ENTRY_BACKFILL_COMPLETE.insert(cache_key);
            return Ok(0);
        };

        let needs_backfill = match existing {
            None => true,
            Some(stats) => {
                stats.count != expected_count
                    || stats.min_index != expected_min
                    || stats.max_index != expected_max
            }
        };

        if !needs_backfill {
            LOG_ENTRY_BACKFILL_COMPLETE.insert(cache_key);
            return Ok(0);
        }

        ExecutionProcessLogEntry::upsert_entries(
            &self.db().pool,
            execution_id,
            channel,
            &entries,
        )
        .await?;

        LOG_ENTRY_BACKFILL_COMPLETE.insert(cache_key);
        Ok(entries.len())
    }

    async fn collect_raw_entries_from_jsonl(
        &self,
        execution_id: Uuid,
    ) -> Result<Vec<LogEntryRow>, ContainerError> {
        let log_records =
            ExecutionProcessLogs::find_by_execution_id(&self.db().pool, execution_id).await?;
        if log_records.is_empty() {
            return Ok(Vec::new());
        }

        let messages = ExecutionProcessLogs::parse_logs(&log_records)
            .map_err(|e| ContainerError::Other(anyhow!("Failed to parse logs: {e}")))?;

        let mut entries: Vec<LogEntryRow> = Vec::new();
        let mut index: i64 = 0;

        for msg in messages {
            match msg {
                LogMsg::Stdout(content) => {
                    let entry_json = serde_json::to_string(&serde_json::json!({
                        "type": "STDOUT",
                        "content": content
                    }))
                    .map_err(|e| ContainerError::Other(anyhow!("Failed to encode entry: {e}")))?;
                    entries.push(LogEntryRow {
                        entry_index: index,
                        entry_json,
                    });
                    index += 1;
                }
                LogMsg::Stderr(content) => {
                    let entry_json = serde_json::to_string(&serde_json::json!({
                        "type": "STDERR",
                        "content": content
                    }))
                    .map_err(|e| ContainerError::Other(anyhow!("Failed to encode entry: {e}")))?;
                    entries.push(LogEntryRow {
                        entry_index: index,
                        entry_json,
                    });
                    index += 1;
                }
                _ => {}
            }
        }

        Ok(entries)
    }

    async fn backfill_raw_entries_from_jsonl(
        &self,
        execution_id: Uuid,
    ) -> Result<usize, ContainerError> {
        let entries = self.collect_raw_entries_from_jsonl(execution_id).await?;
        if entries.is_empty() {
            return Ok(0);
        }

        ExecutionProcessLogEntry::upsert_entries(
            &self.db().pool,
            execution_id,
            LogEntryChannel::Raw,
            &entries,
        )
        .await?;

        Ok(entries.len())
    }

    async fn collect_normalized_entries_from_jsonl(
        &self,
        execution_id: Uuid,
    ) -> Result<Vec<LogEntryRow>, ContainerError> {
        let log_records =
            ExecutionProcessLogs::find_by_execution_id(&self.db().pool, execution_id).await?;
        if log_records.is_empty() {
            return Ok(Vec::new());
        }

        let messages = ExecutionProcessLogs::parse_logs(&log_records)
            .map_err(|e| ContainerError::Other(anyhow!("Failed to parse logs: {e}")))?;

        let mut entries: Vec<LogEntryRow> = Vec::new();
        for msg in &messages {
            if let LogMsg::JsonPatch(patch) = msg {
                entries.extend(extract_normalized_patch_entries(patch));
            }
        }

        if entries.is_empty() {
            if let Some(mut stream) = self.stream_normalized_logs(&execution_id).await {
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(LogMsg::JsonPatch(patch)) => {
                            entries.extend(extract_normalized_patch_entries(&patch));
                        }
                        Ok(LogMsg::Finished) => break,
                        Ok(_) => {}
                        Err(e) => {
                            return Err(ContainerError::Other(anyhow!(
                                "Normalized log stream error: {e}"
                            )));
                        }
                    }
                }
            }
        }

        Ok(dedupe_entries_by_index(entries))
    }

    async fn backfill_normalized_entries_from_jsonl(
        &self,
        execution_id: Uuid,
    ) -> Result<usize, ContainerError> {
        let entries = self.collect_normalized_entries_from_jsonl(execution_id).await?;
        if entries.is_empty() {
            return Ok(0);
        }

        ExecutionProcessLogEntry::upsert_entries(
            &self.db().pool,
            execution_id,
            LogEntryChannel::Normalized,
            &entries,
        )
        .await?;

        Ok(entries.len())
    }

    fn cleanup_actions_for_repos(&self, repos: &[ProjectRepoWithName]) -> Option<ExecutorAction> {
        let repos_with_cleanup: Vec<_> = repos
            .iter()
            .filter(|r| r.cleanup_script.is_some())
            .collect();

        if repos_with_cleanup.is_empty() {
            return None;
        }

        let mut iter = repos_with_cleanup.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.cleanup_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::CleanupScript,
                working_dir: Some(first.repo_name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.cleanup_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::CleanupScript,
                    working_dir: Some(repo.repo_name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    fn setup_actions_for_repos(&self, repos: &[ProjectRepoWithName]) -> Option<ExecutorAction> {
        let repos_with_setup: Vec<_> = repos.iter().filter(|r| r.setup_script.is_some()).collect();

        if repos_with_setup.is_empty() {
            return None;
        }

        let mut iter = repos_with_setup.iter();
        let first = iter.next()?;
        let mut root_action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: first.setup_script.clone().unwrap(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: Some(first.repo_name.clone()),
            }),
            None,
        );

        for repo in iter {
            root_action = root_action.append_action(ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: repo.setup_script.clone().unwrap(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: Some(repo.repo_name.clone()),
                }),
                None,
            ));
        }

        Some(root_action)
    }

    fn setup_action_for_repo(repo: &ProjectRepoWithName) -> Option<ExecutorAction> {
        repo.setup_script.as_ref().map(|script| {
            ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: script.clone(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: Some(repo.repo_name.clone()),
                }),
                None,
            )
        })
    }

    fn build_sequential_setup_chain(
        repos: &[&ProjectRepoWithName],
        next_action: ExecutorAction,
    ) -> ExecutorAction {
        let mut chained = next_action;
        for repo in repos.iter().rev() {
            if let Some(script) = &repo.setup_script {
                chained = ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(ScriptRequest {
                        script: script.clone(),
                        language: ScriptRequestLanguage::Bash,
                        context: ScriptContext::SetupScript,
                        working_dir: Some(repo.repo_name.clone()),
                    }),
                    Some(Box::new(chained)),
                );
            }
        }
        chained
    }

    async fn try_stop(&self, workspace: &Workspace, include_dev_server: bool) {
        // stop execution processes for this workspace's sessions
        let sessions = match Session::find_by_workspace_id(&self.db().pool, workspace.id).await {
            Ok(s) => s,
            Err(_) => return,
        };

        for session in sessions {
            if let Ok(processes) =
                ExecutionProcess::find_by_session_id(&self.db().pool, session.id, false).await
            {
                for process in processes {
                    // Skip dev server processes unless explicitly included
                    if !include_dev_server
                        && process.run_reason == ExecutionProcessRunReason::DevServer
                    {
                        continue;
                    }
                    if process.status == ExecutionProcessStatus::Running {
                        self.stop_execution(&process, ExecutionProcessStatus::Killed)
                            .await
                            .unwrap_or_else(|e| {
                                tracing::debug!(
                                    "Failed to stop execution process {} for workspace {}: {}",
                                    process.id,
                                    workspace.id,
                                    e
                                );
                            });
                    }
                }
            }
        }
    }

    async fn try_stop_force(&self, workspace: &Workspace, include_dev_server: bool) {
        // stop execution processes for this workspace's sessions
        let sessions = match Session::find_by_workspace_id(&self.db().pool, workspace.id).await {
            Ok(s) => s,
            Err(_) => return,
        };

        for session in sessions {
            if let Ok(processes) =
                ExecutionProcess::find_by_session_id(&self.db().pool, session.id, false).await
            {
                for process in processes {
                    // Skip dev server processes unless explicitly included
                    if !include_dev_server
                        && process.run_reason == ExecutionProcessRunReason::DevServer
                    {
                        continue;
                    }
                    if process.status == ExecutionProcessStatus::Running {
                        self.stop_execution_force(&process, ExecutionProcessStatus::Killed)
                            .await
                            .unwrap_or_else(|e| {
                                tracing::debug!(
                                    "Failed to stop execution process {} for workspace {}: {}",
                                    process.id,
                                    workspace.id,
                                    e
                                );
                            });
                    }
                }
            }
        }
    }

    async fn ensure_container_exists(
        &self,
        workspace: &Workspace,
    ) -> Result<ContainerRef, ContainerError>;

    async fn is_container_clean(&self, workspace: &Workspace) -> Result<bool, ContainerError>;

    async fn start_execution_inner(
        &self,
        workspace: &Workspace,
        execution_process: &ExecutionProcess,
        executor_action: &ExecutorAction,
    ) -> Result<(), ContainerError>;

    async fn stop_execution(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError>;

    async fn stop_execution_force(
        &self,
        execution_process: &ExecutionProcess,
        status: ExecutionProcessStatus,
    ) -> Result<(), ContainerError> {
        self.stop_execution(execution_process, status).await
    }

    async fn try_commit_changes(&self, ctx: &ExecutionContext) -> Result<bool, ContainerError>;

    async fn copy_project_files(
        &self,
        source_dir: &Path,
        target_dir: &Path,
        copy_files: &str,
    ) -> Result<(), ContainerError>;

    /// Stream diff updates as LogMsg for WebSocket endpoints.
    async fn stream_diff(
        &self,
        workspace: &Workspace,
        options: DiffStreamOptions,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>, ContainerError>;

    /// Fetch the MsgStore for a given execution ID, panicking if missing.
    async fn get_msg_store_by_id(&self, uuid: &Uuid) -> Option<Arc<MsgStore>> {
        let map = self.msg_stores().read().await;
        map.get(uuid).cloned()
    }

    async fn git_branch_prefix(&self) -> String;

    async fn git_branch_from_workspace(&self, workspace_id: &Uuid, task_title: &str) -> String {
        let task_title_id = git_branch_id(task_title);
        let prefix = self.git_branch_prefix().await;

        if prefix.is_empty() {
            format!("{}-{}", short_uuid(workspace_id), task_title_id)
        } else {
            format!("{}/{}-{}", prefix, short_uuid(workspace_id), task_title_id)
        }
    }

    async fn stream_raw_logs(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>> {
        if let Some(store) = self.get_msg_store_by_id(id).await {
            // First try in-memory store
            return Some(
                store
                    .history_plus_stream()
                    .filter(|msg| {
                        future::ready(matches!(
                            msg,
                            Ok(LogMsg::Stdout(..) | LogMsg::Stderr(..) | LogMsg::Finished)
                        ))
                    })
                    .boxed(),
            );
        } else {
            // Fallback: load from DB and create direct stream
            let log_records =
                match ExecutionProcessLogs::find_by_execution_id(&self.db().pool, *id).await {
                    Ok(records) if !records.is_empty() => records,
                    Ok(_) => return None, // No logs exist
                    Err(e) => {
                        tracing::error!("Failed to fetch logs for execution {}: {}", id, e);
                        return None;
                    }
                };

            let messages = match ExecutionProcessLogs::parse_logs(&log_records) {
                Ok(msgs) => msgs,
                Err(e) => {
                    tracing::error!("Failed to parse logs for execution {}: {}", id, e);
                    return None;
                }
            };

            // Direct stream from parsed messages
            let stream = futures::stream::iter(
                messages
                    .into_iter()
                    .filter(|m| matches!(m, LogMsg::Stdout(_) | LogMsg::Stderr(_)))
                    .chain(std::iter::once(LogMsg::Finished))
                    .map(Ok::<_, std::io::Error>),
            )
            .boxed();

            Some(stream)
        }
    }

    async fn stream_normalized_logs(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogMsg, std::io::Error>>> {
        // First try in-memory store (existing behavior)
        if let Some(store) = self.get_msg_store_by_id(id).await {
            Some(
                store
                    .history_plus_stream() // BoxStream<Result<LogMsg, io::Error>>
                    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)))))
                    .chain(futures::stream::once(async {
                        Ok::<_, std::io::Error>(LogMsg::Finished)
                    }))
                    .boxed(),
            )
        } else {
            // Fallback: load from DB and normalize
            let log_records =
                match ExecutionProcessLogs::find_by_execution_id(&self.db().pool, *id).await {
                    Ok(records) if !records.is_empty() => records,
                    Ok(_) => return None, // No logs exist
                    Err(e) => {
                        tracing::error!("Failed to fetch logs for execution {}: {}", id, e);
                        return None;
                    }
                };

            let raw_messages = match ExecutionProcessLogs::parse_logs(&log_records) {
                Ok(msgs) => msgs,
                Err(e) => {
                    tracing::error!("Failed to parse logs for execution {}: {}", id, e);
                    return None;
                }
            };

            // Create temporary store and populate
            // Include JsonPatch messages (already normalized) and Stdout/Stderr (need normalization)
            let temp_store = Arc::new(MsgStore::new());
            for msg in raw_messages {
                if matches!(
                    msg,
                    LogMsg::Stdout(_) | LogMsg::Stderr(_) | LogMsg::JsonPatch(_)
                ) {
                    temp_store.push(msg);
                }
            }
            temp_store.push_finished();

            let process = match ExecutionProcess::find_by_id(&self.db().pool, *id).await {
                Ok(Some(process)) => process,
                Ok(None) => {
                    tracing::error!("No execution process found for ID: {}", id);
                    return None;
                }
                Err(e) => {
                    tracing::error!("Failed to fetch execution process {}: {}", id, e);
                    return None;
                }
            };

            // Get the workspace to determine correct directory
            let (workspace, _session) =
                match process.parent_workspace_and_session(&self.db().pool).await {
                    Ok(Some((workspace, session))) => (workspace, session),
                    Ok(None) => {
                        tracing::error!(
                            "No workspace/session found for session ID: {}",
                            process.session_id
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to fetch workspace for session {}: {}",
                            process.session_id,
                            e
                        );
                        return None;
                    }
                };

            if let Err(err) = self.ensure_container_exists(&workspace).await {
                tracing::warn!(
                    "Failed to recreate worktree before log normalization for workspace {}: {}",
                    workspace.id,
                    err
                );
            }

            let current_dir = self.workspace_to_current_dir(&workspace);

            let executor_action = if let Ok(executor_action) = process.executor_action() {
                executor_action
            } else {
                tracing::error!(
                    "Failed to parse executor action: {:?}",
                    process.executor_action()
                );
                return None;
            };

            // Spawn normalizer on populated store
            match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => {
                    let executor = ExecutorConfigs::get_cached()
                        .get_coding_agent_or_default(&request.executor_profile_id);
                    executor.normalize_logs(temp_store.clone(), &current_dir);
                }
                ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                    let executor = ExecutorConfigs::get_cached()
                        .get_coding_agent_or_default(&request.executor_profile_id);
                    executor.normalize_logs(temp_store.clone(), &current_dir);
                }
                _ => {
                    tracing::debug!(
                        "Executor action doesn't support log normalization: {:?}",
                        process.executor_action()
                    );
                    return None;
                }
            }
            Some(
                temp_store
                    .history_plus_stream()
                    .filter(|msg| future::ready(matches!(msg, Ok(LogMsg::JsonPatch(..)))))
                    .chain(futures::stream::once(async {
                        Ok::<_, std::io::Error>(LogMsg::Finished)
                    }))
                    .boxed(),
            )
        }
    }

    async fn stream_raw_log_entries(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogEntryEvent, std::io::Error>>>
    {
        self.get_msg_store_by_id(id)
            .await
            .map(|store| store.raw_history_plus_stream())
    }

    async fn stream_normalized_log_entries(
        &self,
        id: &Uuid,
    ) -> Option<futures::stream::BoxStream<'static, Result<LogEntryEvent, std::io::Error>>>
    {
        self.get_msg_store_by_id(id)
            .await
            .map(|store| store.normalized_history_plus_stream())
    }

    async fn log_history_page(
        &self,
        execution_process: &ExecutionProcess,
        channel: LogEntryChannel,
        limit: usize,
        cursor: Option<i64>,
    ) -> Result<LogHistoryPageData, ContainerError> {
        if execution_process.status == ExecutionProcessStatus::Running {
            if let Some(store) = self.get_msg_store_by_id(&execution_process.id).await {
                let cursor = cursor.and_then(|c| usize::try_from(c).ok());
                let (entries, has_more) = match channel {
                    LogEntryChannel::Raw => store.raw_history_page(limit, cursor),
                    LogEntryChannel::Normalized => store.normalized_history_page(limit, cursor),
                };
                return Ok(LogHistoryPageData { entries, has_more });
            }
        }

        if execution_process.status != ExecutionProcessStatus::Running {
            self.backfill_log_entries_if_incomplete(execution_process.id, channel)
                .await?;
        }

        if ExecutionProcessLogEntry::has_any(&self.db().pool, execution_process.id, channel).await?
        {
            let mut rows = ExecutionProcessLogEntry::fetch_page(
                &self.db().pool,
                execution_process.id,
                channel,
                limit,
                cursor,
            )
            .await?;

            rows.reverse();

            let entries = rows
                .into_iter()
                .filter_map(|row| match serde_json::from_str::<serde_json::Value>(&row.entry_json) {
                    Ok(entry_json) => Some(LogEntrySnapshot {
                        entry_index: row.entry_index as usize,
                        entry_json,
                    }),
                    Err(err) => {
                        tracing::warn!(
                            "Failed to parse log entry {} for {}: {}",
                            row.entry_index,
                            execution_process.id,
                            err
                        );
                        None
                    }
                })
                .collect::<Vec<_>>();

            let has_more = if let Some(first) = entries.first() {
                ExecutionProcessLogEntry::has_older(
                    &self.db().pool,
                    execution_process.id,
                    channel,
                    first.entry_index as i64,
                )
                .await?
            } else {
                false
            };

            return Ok(LogHistoryPageData { entries, has_more });
        }

        if let Some(store) = self.get_msg_store_by_id(&execution_process.id).await {
            let cursor = cursor.and_then(|c| usize::try_from(c).ok());
            let (entries, has_more) = match channel {
                LogEntryChannel::Raw => store.raw_history_page(limit, cursor),
                LogEntryChannel::Normalized => store.normalized_history_page(limit, cursor),
            };
            return Ok(LogHistoryPageData { entries, has_more });
        }

        Ok(LogHistoryPageData {
            entries: Vec::new(),
            has_more: false,
        })
    }

    fn spawn_stream_raw_logs_to_db(&self, execution_id: &Uuid) -> JoinHandle<()> {
        let execution_id = *execution_id;
        let msg_stores = self.msg_stores().clone();
        let db = self.db().clone();

        tokio::spawn(async move {
            // Get the message store for this execution
            let store = {
                let map = msg_stores.read().await;
                map.get(&execution_id).cloned()
            };

            if let Some(store) = store {
                let mut stream = store.history_plus_stream();

                while let Some(Ok(msg)) = stream.next().await {
                    match &msg {
                        LogMsg::Stdout(_) | LogMsg::Stderr(_) => {
                            // Serialize this individual message as a JSONL line
                            match serde_json::to_string(&msg) {
                                Ok(jsonl_line) => {
                                    let jsonl_line_with_newline = format!("{jsonl_line}\n");

                                    // Append this line to the database
                                    if let Err(e) = ExecutionProcessLogs::append_log_line(
                                        &db.pool,
                                        execution_id,
                                        &jsonl_line_with_newline,
                                    )
                                    .await
                                    {
                                        tracing::error!(
                                            "Failed to append log line for execution {}: {}",
                                            execution_id,
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to serialize log message for execution {}: {}",
                                        execution_id,
                                        e
                                    );
                                }
                            }
                        }
                        LogMsg::SessionId(agent_session_id) => {
                            // Append this line to the database
                            if let Err(e) = CodingAgentTurn::update_agent_session_id(
                                &db.pool,
                                execution_id,
                                agent_session_id,
                            )
                            .await
                            {
                                tracing::error!(
                                    "Failed to update agent_session_id {} for execution process {}: {}",
                                    agent_session_id,
                                    execution_id,
                                    e
                                );
                            }
                        }
                        LogMsg::Finished => {
                            break;
                        }
                        LogMsg::JsonPatch(_) => continue,
                    }
                }
            }
        })
    }

    fn spawn_stream_raw_entries_to_db(&self, execution_id: &Uuid) -> JoinHandle<()> {
        let execution_id = *execution_id;
        let msg_stores = self.msg_stores().clone();
        let db = self.db().clone();

        tokio::spawn(async move {
            let store = {
                let map = msg_stores.read().await;
                map.get(&execution_id).cloned()
            };

            let Some(store) = store else {
                return;
            };

            let mut stream = store.raw_history_plus_stream();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(LogEntryEvent::Append { entry_index, entry })
                    | Ok(LogEntryEvent::Replace { entry_index, entry }) => {
                        let entry_json = match serde_json::to_string(&entry) {
                            Ok(json) => json,
                            Err(err) => {
                                tracing::warn!(
                                    "Failed to encode raw log entry {} for {}: {}",
                                    entry_index,
                                    execution_id,
                                    err
                                );
                                continue;
                            }
                        };

                        if let Err(err) = ExecutionProcessLogEntry::upsert_entry(
                            &db.pool,
                            execution_id,
                            LogEntryChannel::Raw,
                            entry_index as i64,
                            &entry_json,
                        )
                        .await
                        {
                            tracing::error!(
                                "Failed to persist raw log entry {} for {}: {}",
                                entry_index,
                                execution_id,
                                err
                            );
                        }
                    }
                    Ok(LogEntryEvent::Finished) => break,
                    Err(err) => {
                        tracing::error!("raw entry stream error: {}", err);
                        break;
                    }
                }
            }
        })
    }

    fn spawn_stream_normalized_entries_to_db(&self, execution_id: &Uuid) -> JoinHandle<()> {
        let execution_id = *execution_id;
        let msg_stores = self.msg_stores().clone();
        let db = self.db().clone();

        tokio::spawn(async move {
            let store = {
                let map = msg_stores.read().await;
                map.get(&execution_id).cloned()
            };

            let Some(store) = store else {
                return;
            };

            let mut stream = store.normalized_history_plus_stream();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(LogEntryEvent::Append { entry_index, entry })
                    | Ok(LogEntryEvent::Replace { entry_index, entry }) => {
                        let entry_json = match serde_json::to_string(&entry) {
                            Ok(json) => json,
                            Err(err) => {
                                tracing::warn!(
                                    "Failed to encode normalized log entry {} for {}: {}",
                                    entry_index,
                                    execution_id,
                                    err
                                );
                                continue;
                            }
                        };

                        if let Err(err) = ExecutionProcessLogEntry::upsert_entry(
                            &db.pool,
                            execution_id,
                            LogEntryChannel::Normalized,
                            entry_index as i64,
                            &entry_json,
                        )
                        .await
                        {
                            tracing::error!(
                                "Failed to persist normalized log entry {} for {}: {}",
                                entry_index,
                                execution_id,
                                err
                            );
                        }
                    }
                    Ok(LogEntryEvent::Finished) => break,
                    Err(err) => {
                        tracing::error!("normalized entry stream error: {}", err);
                        break;
                    }
                }
            }
        })
    }

    async fn start_workspace(
        &self,
        workspace: &Workspace,
        executor_profile_id: ExecutorProfileId,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Create container
        self.create(workspace).await?;

        // Get parent task
        let task = workspace
            .parent_task(&self.db().pool)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        // Get parent project
        let project = task
            .parent_project(&self.db().pool)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        let project_repos =
            ProjectRepo::find_by_project_id_with_names(&self.db().pool, project.id).await?;

        let workspace = Workspace::find_by_id(&self.db().pool, workspace.id)
            .await?
            .ok_or(SqlxError::RowNotFound)?;

        // Create a session for this workspace
        let session = Session::create(
            &self.db().pool,
            &CreateSession {
                executor: Some(executor_profile_id.executor.to_string()),
            },
            Uuid::new_v4(),
            workspace.id,
        )
        .await?;

        let prompt = task.to_prompt();

        let repos_with_setup: Vec<_> = project_repos
            .iter()
            .filter(|pr| pr.setup_script.is_some())
            .collect();

        let all_parallel = repos_with_setup.iter().all(|pr| pr.parallel_setup_script);

        let cleanup_action = self.cleanup_actions_for_repos(&project_repos);

        let working_dir = workspace
            .agent_working_dir
            .as_ref()
            .filter(|dir| !dir.is_empty())
            .cloned();

        let coding_action = ExecutorAction::new(
            ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                prompt,
                executor_profile_id: executor_profile_id.clone(),
                working_dir,
            }),
            cleanup_action.map(Box::new),
        );

        let execution_process = if all_parallel {
            // All parallel: start each setup independently, then start coding agent
            for repo in &repos_with_setup {
                if let Some(action) = Self::setup_action_for_repo(repo)
                    && let Err(e) = self
                        .start_execution(
                            &workspace,
                            &session,
                            &action,
                            &ExecutionProcessRunReason::SetupScript,
                        )
                        .await
                {
                    tracing::warn!(?e, "Failed to start setup script in parallel mode");
                }
            }
            self.start_execution(
                &workspace,
                &session,
                &coding_action,
                &ExecutionProcessRunReason::CodingAgent,
            )
            .await?
        } else {
            // Any sequential: chain ALL setups → coding agent via next_action
            let main_action = Self::build_sequential_setup_chain(&repos_with_setup, coding_action);
            self.start_execution(
                &workspace,
                &session,
                &main_action,
                &ExecutionProcessRunReason::SetupScript,
            )
            .await?
        };

        Ok(execution_process)
    }

    async fn start_execution(
        &self,
        workspace: &Workspace,
        session: &Session,
        executor_action: &ExecutorAction,
        run_reason: &ExecutionProcessRunReason,
    ) -> Result<ExecutionProcess, ContainerError> {
        // Update task status to InProgress when starting an execution
        let task = workspace
            .parent_task(&self.db().pool)
            .await?
            .ok_or(SqlxError::RowNotFound)?;
        if task.status != TaskStatus::InProgress
            && run_reason != &ExecutionProcessRunReason::DevServer
        {
            Task::update_status(&self.db().pool, task.id, TaskStatus::InProgress).await?;
        }
        // Create new execution process record
        // Capture current HEAD per repository as the "before" commit for this execution
        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db().pool, workspace.id).await?;
        if repositories.is_empty() {
            return Err(ContainerError::Other(anyhow!(
                "Workspace has no repositories configured"
            )));
        }

        let workspace_root = workspace
            .container_ref
            .as_ref()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| ContainerError::Other(anyhow!("Container ref not found")))?;

        let mut repo_states = Vec::with_capacity(repositories.len());
        for repo in &repositories {
            let repo_path = workspace_root.join(&repo.name);
            let before_head_commit = self.git().get_head_info(&repo_path).ok().map(|h| h.oid);
            repo_states.push(CreateExecutionProcessRepoState {
                repo_id: repo.id,
                before_head_commit,
                after_head_commit: None,
                merge_commit: None,
            });
        }
        let create_execution_process = CreateExecutionProcess {
            session_id: session.id,
            executor_action: executor_action.clone(),
            run_reason: run_reason.clone(),
        };

        let execution_process = ExecutionProcess::create(
            &self.db().pool,
            &create_execution_process,
            Uuid::new_v4(),
            &repo_states,
        )
        .await?;

        if let Some(prompt) = match executor_action.typ() {
            ExecutorActionType::CodingAgentInitialRequest(coding_agent_request) => {
                Some(coding_agent_request.prompt.clone())
            }
            ExecutorActionType::CodingAgentFollowUpRequest(follow_up_request) => {
                Some(follow_up_request.prompt.clone())
            }
            _ => None,
        } {
            let create_coding_agent_turn = CreateCodingAgentTurn {
                execution_process_id: execution_process.id,
                prompt: Some(prompt),
            };

            let coding_agent_turn_id = Uuid::new_v4();

            CodingAgentTurn::create(
                &self.db().pool,
                &create_coding_agent_turn,
                coding_agent_turn_id,
            )
            .await?;
        }

        if let Err(start_error) = self
            .start_execution_inner(workspace, &execution_process, executor_action)
            .await
        {
            // Mark process as failed
            if let Err(update_error) = ExecutionProcess::update_completion(
                &self.db().pool,
                execution_process.id,
                ExecutionProcessStatus::Failed,
                None,
            )
            .await
            {
                tracing::error!(
                    "Failed to mark execution process {} as failed after start error: {}",
                    execution_process.id,
                    update_error
                );
            }
            Task::update_status(&self.db().pool, task.id, TaskStatus::InReview).await?;

            // Emit stderr error message
            let log_message = LogMsg::Stderr(format!("Failed to start execution: {start_error}"));
            if let Ok(json_line) = serde_json::to_string(&log_message) {
                let _ = ExecutionProcessLogs::append_log_line(
                    &self.db().pool,
                    execution_process.id,
                    &format!("{json_line}\n"),
                )
                .await;
            }

            // Emit NextAction with failure context for coding agent requests
            if let ContainerError::ExecutorError(ExecutorError::ExecutableNotFound { program }) =
                &start_error
            {
                let help_text = format!("The required executable `{program}` is not installed.");
                let error_message = NormalizedEntry {
                    timestamp: None,
                    entry_type: NormalizedEntryType::ErrorMessage {
                        error_type: NormalizedEntryError::SetupRequired,
                    },
                    content: help_text,
                    metadata: None,
                };
                let patch = ConversationPatch::add_normalized_entry(2, error_message);
                if let Ok(json_line) = serde_json::to_string::<LogMsg>(&LogMsg::JsonPatch(patch)) {
                    let _ = ExecutionProcessLogs::append_log_line(
                        &self.db().pool,
                        execution_process.id,
                        &format!("{json_line}\n"),
                    )
                    .await;
                }
            };
            return Err(start_error);
        }

        // Start processing normalised logs for executor requests and follow ups
        if let Some(msg_store) = self.get_msg_store_by_id(&execution_process.id).await
            && let Some(executor_profile_id) = match executor_action.typ() {
                ExecutorActionType::CodingAgentInitialRequest(request) => {
                    Some(&request.executor_profile_id)
                }
                ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                    Some(&request.executor_profile_id)
                }
                _ => None,
            }
        {
            if let Some(executor) =
                ExecutorConfigs::get_cached().get_coding_agent(executor_profile_id)
            {
                executor.normalize_logs(msg_store, &self.workspace_to_current_dir(workspace));
            } else {
                tracing::error!(
                    "Failed to resolve profile '{:?}' for normalization",
                    executor_profile_id
                );
            }
        }

        self.spawn_stream_raw_logs_to_db(&execution_process.id);
        self.spawn_stream_raw_entries_to_db(&execution_process.id);
        self.spawn_stream_normalized_entries_to_db(&execution_process.id);
        Ok(execution_process)
    }

    async fn try_start_next_action(&self, ctx: &ExecutionContext) -> Result<(), ContainerError> {
        let action = ctx.execution_process.executor_action()?;
        let next_action = if let Some(next_action) = action.next_action() {
            next_action
        } else {
            tracing::debug!("No next action configured");
            return Ok(());
        };

        // Determine the run reason of the next action
        let next_run_reason = match (action.typ(), next_action.typ()) {
            (ExecutorActionType::ScriptRequest(_), ExecutorActionType::ScriptRequest(_)) => {
                ExecutionProcessRunReason::SetupScript
            }
            (
                ExecutorActionType::CodingAgentInitialRequest(_)
                | ExecutorActionType::CodingAgentFollowUpRequest(_),
                ExecutorActionType::ScriptRequest(_),
            ) => ExecutionProcessRunReason::CleanupScript,
            (
                _,
                ExecutorActionType::CodingAgentFollowUpRequest(_)
                | ExecutorActionType::CodingAgentInitialRequest(_),
            ) => ExecutionProcessRunReason::CodingAgent,
        };

        self.start_execution(&ctx.workspace, &ctx.session, next_action, &next_run_reason)
            .await?;

        tracing::debug!("Started next action: {:?}", next_action);
        Ok(())
    }
}

fn extract_normalized_patch_entries(patch: &json_patch::Patch) -> Vec<LogEntryRow> {
    patch
        .iter()
        .filter_map(|op| match op {
            json_patch::PatchOperation::Add(add) => {
                normalized_entry_from_patch(&add.path, &add.value)
            }
            json_patch::PatchOperation::Replace(replace) => {
                normalized_entry_from_patch(&replace.path, &replace.value)
            }
            _ => None,
        })
        .collect()
}

fn normalized_entry_from_patch(path: &str, value: &serde_json::Value) -> Option<LogEntryRow> {
    let index = path.strip_prefix("/entries/")?.parse::<i64>().ok()?;
    let entry_type = value.get("type")?.as_str()?;
    if entry_type != "NORMALIZED_ENTRY" {
        return None;
    }

    let entry_json = serde_json::to_string(value).ok()?;
    Some(LogEntryRow {
        entry_index: index,
        entry_json,
    })
}

fn dedupe_entries_by_index(entries: Vec<LogEntryRow>) -> Vec<LogEntryRow> {
    let mut map: BTreeMap<i64, String> = BTreeMap::new();
    for entry in entries {
        map.insert(entry.entry_index, entry.entry_json);
    }

    map.into_iter()
        .map(|(entry_index, entry_json)| LogEntryRow {
            entry_index,
            entry_json,
        })
        .collect()
}

fn entry_stats(entries: &[LogEntryRow]) -> Option<(i64, i64, i64)> {
    if entries.is_empty() {
        return None;
    }

    let mut min_index = entries[0].entry_index;
    let mut max_index = entries[0].entry_index;
    for entry in entries.iter().skip(1) {
        min_index = min_index.min(entry.entry_index);
        max_index = max_index.max(entry.entry_index);
    }

    Some((entries.len() as i64, min_index, max_index))
}
