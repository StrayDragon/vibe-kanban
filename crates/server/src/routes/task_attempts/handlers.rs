use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use axum::{
    Extension, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Json as ResponseJson,
};
use db::{
    DbErr, DbPool, TransactionTrait,
    models::{
        execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
        merge::{Merge, MergeStatus, PrMerge, PullRequestInfo},
        project_repo::ProjectRepo,
        repo::{Repo, RepoError},
        session::{CreateSession, Session},
        task::{Task, TaskRelationships, TaskStatus},
        task_group::{
            TaskGroup, TaskGroupError, TaskGroupGraph, TaskGroupNode, TaskGroupNodeBaseStrategy,
        },
        workspace::{CreateWorkspace, Workspace, WorkspaceError},
        workspace_repo::{CreateWorkspaceRepo, RepoWithTargetBranch, WorkspaceRepo},
    },
};
use deployment::Deployment;
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
    executors::{CodingAgent, ExecutorError},
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use git2::BranchType;
use services::services::{
    container::ContainerService,
    diff_stream,
    git::{
        ConflictOp, DiffContentPolicy, DiffTarget, GitCliError, GitMergeOptions, GitService,
        GitServiceError,
    },
    github::GitHubService,
};
use utils::{
    diff::{create_unified_diff, DiffSummary},
    response::ApiResponse,
    text::truncate_to_char_boundary,
};
use uuid::Uuid;

use super::{codex_setup, cursor_setup, dto::*, gh_cli_setup};
use crate::{
    DeploymentImpl, error::ApiError, routes::task_attempts::gh_cli_setup::GhCliSetupError,
};

async fn run_git_operation<T, F>(git: GitService, op: F) -> Result<T, GitServiceError>
where
    T: Send + 'static,
    F: FnOnce(GitService) -> Result<T, GitServiceError> + Send + 'static,
{
    tokio::task::spawn_blocking(move || op(git))
        .await
        .map_err(|err| GitServiceError::InvalidRepository(format!("Git task join failed: {err}")))?
}

fn validate_dev_server_script(script: &str) -> Result<(), ApiError> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest(
            "No dev server script configured for this project".to_string(),
        ));
    }

    let parts = shlex::split(trimmed)
        .ok_or_else(|| ApiError::BadRequest("Dev script is not valid command text".to_string()))?;
    if parts.is_empty() {
        return Err(ApiError::BadRequest(
            "Dev script command is empty".to_string(),
        ));
    }
    let has_forbidden_shell_operators = parts.iter().any(|part| {
        matches!(
            part.as_str(),
            "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<" | "<<"
        )
    });
    if has_forbidden_shell_operators {
        return Err(ApiError::BadRequest(
            "Dev script must be a single command without shell operators".to_string(),
        ));
    }
    Ok(())
}

fn normalize_dev_server_working_dir(
    workspace_root: &Path,
    configured_working_dir: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(raw_working_dir) = configured_working_dir
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
    else {
        return Ok(None);
    };
    let relative = PathBuf::from(raw_working_dir);
    if relative.is_absolute() {
        return Err(ApiError::BadRequest(
            "Dev script working directory must be relative to the workspace root".to_string(),
        ));
    }

    let workspace_root = std::fs::canonicalize(workspace_root).map_err(ApiError::Io)?;
    let resolved = std::fs::canonicalize(workspace_root.join(&relative)).map_err(|_| {
        ApiError::BadRequest(
            "Dev script working directory does not exist in the workspace".to_string(),
        )
    })?;
    if !resolved.starts_with(&workspace_root) {
        return Err(ApiError::Forbidden(
            "Dev script working directory is outside the workspace root".to_string(),
        ));
    }
    if !resolved.is_dir() {
        return Err(ApiError::BadRequest(
            "Dev script working directory must be a directory".to_string(),
        ));
    }

    let relative_normalized = resolved
        .strip_prefix(&workspace_root)
        .map_err(|_| ApiError::Internal("Failed to normalize working directory".to_string()))?;
    if relative_normalized.as_os_str().is_empty() {
        return Ok(None);
    }

    Ok(Some(relative_normalized.to_string_lossy().to_string()))
}

pub async fn get_task_attempts(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskAttemptQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<Workspace>>>, ApiError> {
    let pool = &deployment.db().pool;
    let workspaces = Workspace::fetch_all(pool, query.task_id).await?;
    Ok(ResponseJson(ApiResponse::success(workspaces)))
}

pub async fn get_task_attempts_with_latest_session(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskAttemptQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<WorkspaceWithSession>>>, ApiError> {
    let pool = &deployment.db().pool;
    let workspaces = Workspace::fetch_all(pool, query.task_id).await?;
    let workspace_ids: Vec<Uuid> = workspaces.iter().map(|workspace| workspace.id).collect();
    let sessions_by_workspace = Session::find_latest_by_workspace_ids(pool, &workspace_ids).await?;

    let attempts = workspaces
        .into_iter()
        .map(|workspace| WorkspaceWithSession {
            session: sessions_by_workspace.get(&workspace.id).cloned(),
            workspace,
        })
        .collect();

    Ok(ResponseJson(ApiResponse::success(attempts)))
}

pub async fn get_task_attempts_latest_summaries(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<TaskAttemptLatestSummaryRequest>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskAttemptLatestSummary>>>, ApiError> {
    if payload.task_ids.is_empty() {
        return Ok(ResponseJson(ApiResponse::success(Vec::new())));
    }

    let mut seen = std::collections::HashSet::new();
    let task_ids: Vec<Uuid> = payload
        .task_ids
        .into_iter()
        .filter(|task_id| seen.insert(*task_id))
        .collect();

    let pool = &deployment.db().pool;
    let workspaces = Workspace::fetch_all_by_task_ids(pool, &task_ids).await?;
    let workspace_ids: Vec<Uuid> = workspaces.iter().map(|workspace| workspace.id).collect();
    let sessions_by_workspace = Session::find_latest_by_workspace_ids(pool, &workspace_ids).await?;

    let mut latest_by_task: HashMap<Uuid, Workspace> = HashMap::new();
    for workspace in workspaces {
        match latest_by_task.entry(workspace.task_id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(workspace);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let current = entry.get();
                let created_cmp = workspace.created_at.cmp(&current.created_at);
                let is_newer = if created_cmp == std::cmp::Ordering::Equal {
                    workspace.id < current.id
                } else {
                    created_cmp == std::cmp::Ordering::Greater
                };

                if is_newer {
                    entry.insert(workspace);
                }
            }
        }
    }

    let mut summaries = Vec::with_capacity(task_ids.len());
    for task_id in task_ids {
        if let Some(workspace) = latest_by_task.get(&task_id) {
            let session = sessions_by_workspace.get(&workspace.id);
            summaries.push(TaskAttemptLatestSummary {
                task_id,
                latest_attempt_id: Some(workspace.id),
                latest_workspace_branch: Some(workspace.branch.clone()),
                latest_session_id: session.map(|s| s.id),
                latest_session_executor: session.and_then(|s| s.executor.clone()),
            });
        } else {
            summaries.push(TaskAttemptLatestSummary {
                task_id,
                latest_attempt_id: None,
                latest_workspace_branch: None,
                latest_session_id: None,
                latest_session_executor: None,
            });
        }
    }

    Ok(ResponseJson(ApiResponse::success(summaries)))
}

pub async fn get_task_attempt(
    Extension(workspace): Extension<Workspace>,
) -> Result<ResponseJson<ApiResponse<Workspace>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(workspace)))
}

pub async fn get_task_attempt_status(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TaskAttemptStatusResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    let latest_session = Session::find_latest_by_workspace_id(pool, workspace.id).await?;

    let mut latest_process: Option<ExecutionProcess> = None;
    for run_reason in [
        ExecutionProcessRunReason::CodingAgent,
        ExecutionProcessRunReason::SetupScript,
        ExecutionProcessRunReason::CleanupScript,
    ] {
        let Some(process) = ExecutionProcess::find_latest_by_workspace_and_run_reason(
            pool,
            workspace.id,
            &run_reason,
        )
        .await?
        else {
            continue;
        };

        let replace = match &latest_process {
            Some(existing) => process.created_at > existing.created_at,
            None => true,
        };
        if replace {
            latest_process = Some(process);
        }
    }

    let (state, failure_summary) = match latest_process.as_ref().map(|p| p.status.clone()) {
        None => (AttemptState::Idle, None),
        Some(ExecutionProcessStatus::Running) => (AttemptState::Running, None),
        Some(ExecutionProcessStatus::Completed) => (AttemptState::Completed, None),
        Some(ExecutionProcessStatus::Failed) => (
            AttemptState::Failed,
            Some(match latest_process.as_ref().and_then(|p| p.exit_code) {
                Some(exit_code) => format!("failed (exit_code={exit_code})"),
                None => "failed".to_string(),
            }),
        ),
        Some(ExecutionProcessStatus::Killed) => (AttemptState::Failed, Some("killed".to_string())),
    };

    let last_activity_at = latest_process
        .as_ref()
        .map(|process| {
            if let Some(completed_at) = process.completed_at {
                completed_at.max(process.updated_at)
            } else {
                process.updated_at
            }
        })
        .or_else(|| latest_session.as_ref().map(|session| session.updated_at));

    let status = TaskAttemptStatusResponse {
        attempt_id: workspace.id,
        task_id: workspace.task_id,
        workspace_branch: workspace.branch,
        created_at: workspace.created_at,
        updated_at: workspace.updated_at,
        latest_session_id: latest_session.as_ref().map(|session| session.id),
        latest_execution_process_id: latest_process.as_ref().map(|process| process.id),
        state,
        last_activity_at,
        failure_summary,
    };

    Ok(ResponseJson(ApiResponse::success(status)))
}

pub async fn get_task_attempt_changes(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<AttemptChangesQuery>,
) -> Result<ResponseJson<ApiResponse<TaskAttemptChangesResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let guard_preset = deployment.config().read().await.diff_preview_guard.clone();
    let force = query.force;

    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id).await?;
    let target_branches: HashMap<_, _> = workspace_repos
        .iter()
        .map(|wr| (wr.repo_id, wr.target_branch.clone()))
        .collect();

    let repositories = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;

    let workspace_root = match workspace
        .container_ref
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        Some(path) => path,
        None => match deployment
            .container()
            .ensure_container_exists(&workspace)
            .await
        {
            Ok(container_ref) => PathBuf::from(container_ref),
            Err(err) => {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    error = %err,
                    "Failed to ensure workspace container for attempt changes"
                );
                let response = TaskAttemptChangesResponse {
                    summary: DiffSummary::default(),
                    blocked: true,
                    blocked_reason: Some(AttemptChangesBlockedReason::SummaryFailed),
                    files: Vec::new(),
                };
                return Ok(ResponseJson(ApiResponse::success(response)));
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
                workspace_id = %workspace.id,
                repo_name = %repo.name,
                "Skipping attempt changes for repo: no target branch configured"
            );
            skipped_repos += 1;
            continue;
        };

        let base_commit = match deployment
            .git()
            .get_base_commit(&repo.path, branch, target_branch)
        {
            Ok(commit) => commit,
            Err(err) => {
                tracing::warn!(
                    workspace_id = %workspace.id,
                    repo_name = %repo.name,
                    error = %err,
                    "Skipping attempt changes for repo: failed to get base commit"
                );
                skipped_repos += 1;
                continue;
            }
        };

        repo_inputs.push((repo, worktree_path, base_commit));
    }

    if repo_inputs.is_empty() {
        let blocked_reason = if total_repos > 0 && skipped_repos == total_repos {
            Some(AttemptChangesBlockedReason::SummaryFailed)
        } else {
            None
        };
        let blocked = blocked_reason.is_some();
        let response = TaskAttemptChangesResponse {
            summary: DiffSummary::default(),
            blocked,
            blocked_reason,
            files: Vec::new(),
        };
        return Ok(ResponseJson(ApiResponse::success(response)));
    }

    let mut summary = DiffSummary::default();
    let mut summary_failed = false;
    for (repo, worktree_path, base_commit) in &repo_inputs {
        match deployment
            .git()
            .get_worktree_diff_summary(worktree_path, base_commit, None)
        {
            Ok(repo_summary) => {
                summary.file_count = summary.file_count.saturating_add(repo_summary.file_count);
                summary.added = summary.added.saturating_add(repo_summary.added);
                summary.deleted = summary.deleted.saturating_add(repo_summary.deleted);
                summary.total_bytes = summary.total_bytes.saturating_add(repo_summary.total_bytes);
            }
            Err(err) => {
                summary_failed = true;
                tracing::warn!(
                    workspace_id = %workspace.id,
                    repo_name = %repo.name,
                    error = %err,
                    "Failed to compute diff summary for attempt changes"
                );
            }
        }
    }

    let guard_enabled = diff_stream::diff_preview_guard_thresholds(guard_preset.clone()).is_some();
    let blocked = !force
        && guard_enabled
        && (summary_failed || diff_stream::diff_preview_guard_exceeded(&summary, guard_preset));
    let blocked_reason = if blocked {
        if summary_failed {
            Some(AttemptChangesBlockedReason::SummaryFailed)
        } else {
            Some(AttemptChangesBlockedReason::ThresholdExceeded)
        }
    } else {
        None
    };

    let mut files: Vec<String> = Vec::new();
    if !blocked {
        let mut seen = std::collections::BTreeSet::new();
        for (repo, worktree_path, base_commit) in &repo_inputs {
            match deployment.git().get_diffs(
                DiffTarget::Worktree {
                    worktree_path,
                    base_commit,
                },
                None,
                DiffContentPolicy::OmitContents,
            ) {
                Ok(diffs) => {
                    for diff in diffs {
                        let Some(path) = diff.new_path.or(diff.old_path) else {
                            continue;
                        };
                        seen.insert(format!("{}/{}", repo.name, path));
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        workspace_id = %workspace.id,
                        repo_name = %repo.name,
                        error = %err,
                        "Failed to compute changed files for attempt changes"
                    );
                }
            }
        }

        files = seen.into_iter().collect();
    }

    let response = TaskAttemptChangesResponse {
        summary,
        blocked,
        blocked_reason,
        files,
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

pub async fn get_task_attempt_file(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<AttemptFileQuery>,
) -> Result<ResponseJson<ApiResponse<AttemptFileResponse>>, ApiError> {
    const DEFAULT_MAX_BYTES: usize = 64 * 1024;
    const HARD_MAX_BYTES: usize = 512 * 1024;

    let path = query
        .path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::BadRequest("path is required".to_string()))?;

    let start = query.start.unwrap_or(0);
    let requested_max_bytes = query.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    if requested_max_bytes > HARD_MAX_BYTES {
        return Ok(ResponseJson(ApiResponse::success(AttemptFileResponse {
            path: path.to_string(),
            blocked: true,
            blocked_reason: Some(AttemptArtifactBlockedReason::SizeExceeded),
            truncated: false,
            start,
            bytes: 0,
            total_bytes: None,
            content: None,
        })));
    }

    let workspace_root = match workspace
        .container_ref
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        Some(path) => path,
        None => PathBuf::from(deployment.container().ensure_container_exists(&workspace).await?),
    };
    let canonical_root = std::fs::canonicalize(&workspace_root).map_err(ApiError::Io)?;

    let rel_path = PathBuf::from(path);
    let invalid = rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
    if invalid {
        return Ok(ResponseJson(ApiResponse::success(AttemptFileResponse {
            path: path.to_string(),
            blocked: true,
            blocked_reason: Some(AttemptArtifactBlockedReason::PathOutsideWorkspace),
            truncated: false,
            start,
            bytes: 0,
            total_bytes: None,
            content: None,
        })));
    }

    let requested_path = workspace_root.join(&rel_path);
    if !requested_path.exists() {
        return Err(ApiError::NotFound("File does not exist".to_string()));
    }
    if !requested_path.is_file() {
        return Err(ApiError::BadRequest("Path is not a file".to_string()));
    }

    let canonical_file = std::fs::canonicalize(&requested_path).map_err(ApiError::Io)?;
    if !canonical_file.starts_with(&canonical_root) {
        return Ok(ResponseJson(ApiResponse::success(AttemptFileResponse {
            path: path.to_string(),
            blocked: true,
            blocked_reason: Some(AttemptArtifactBlockedReason::PathOutsideWorkspace),
            truncated: false,
            start,
            bytes: 0,
            total_bytes: None,
            content: None,
        })));
    }

    let meta = std::fs::metadata(&canonical_file).map_err(ApiError::Io)?;
    let total_bytes = meta.len();
    if start >= total_bytes {
        return Ok(ResponseJson(ApiResponse::success(AttemptFileResponse {
            path: path.to_string(),
            blocked: false,
            blocked_reason: None,
            truncated: false,
            start,
            bytes: 0,
            total_bytes: Some(total_bytes),
            content: Some(String::new()),
        })));
    }

    let read_len = requested_max_bytes.min((total_bytes - start) as usize);
    let mut file = std::fs::File::open(&canonical_file).map_err(ApiError::Io)?;
    file.seek(SeekFrom::Start(start)).map_err(ApiError::Io)?;
    let mut buf = vec![0u8; read_len];
    let n = file.read(&mut buf).map_err(ApiError::Io)?;
    buf.truncate(n);

    let truncated = (start as u128).saturating_add(n as u128) < (total_bytes as u128);
    let content = String::from_utf8_lossy(&buf).into_owned();

    Ok(ResponseJson(ApiResponse::success(AttemptFileResponse {
        path: path.to_string(),
        blocked: false,
        blocked_reason: None,
        truncated,
        start,
        bytes: n,
        total_bytes: Some(total_bytes),
        content: Some(content),
    })))
}

pub async fn get_task_attempt_patch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<AttemptPatchRequest>,
) -> Result<ResponseJson<ApiResponse<AttemptPatchResponse>>, ApiError> {
    const DEFAULT_MAX_BYTES: usize = 200 * 1024;
    const HARD_MAX_BYTES: usize = 2 * 1024 * 1024;
    const MAX_PATHS: usize = 100;

    if request.paths.is_empty() {
        return Err(ApiError::BadRequest("paths must not be empty".to_string()));
    }

    if request.paths.len() > MAX_PATHS {
        return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
            blocked: true,
            blocked_reason: Some(AttemptArtifactBlockedReason::TooManyPaths),
            truncated: false,
            bytes: 0,
            paths: request.paths,
            patch: None,
        })));
    }

    let max_bytes = request.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    if max_bytes > HARD_MAX_BYTES {
        return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
            blocked: true,
            blocked_reason: Some(AttemptArtifactBlockedReason::SizeExceeded),
            truncated: false,
            bytes: 0,
            paths: request.paths,
            patch: None,
        })));
    }

    let pool = &deployment.db().pool;
    let guard_preset = deployment.config().read().await.diff_preview_guard.clone();

    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id).await?;
    let target_branches: HashMap<_, _> = workspace_repos
        .iter()
        .map(|wr| (wr.repo_id, wr.target_branch.clone()))
        .collect();

    let repositories = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;

    let workspace_root = match workspace
        .container_ref
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        Some(path) => path,
        None => PathBuf::from(deployment.container().ensure_container_exists(&workspace).await?),
    };

    let mut repo_inputs = Vec::new();
    let mut summary = DiffSummary::default();
    let mut summary_failed = false;

    for repo in repositories {
        let worktree_path = workspace_root.join(&repo.name);
        let branch = &workspace.branch;

        let Some(target_branch) = target_branches.get(&repo.id) else {
            summary_failed = true;
            continue;
        };

        let base_commit = match deployment
            .git()
            .get_base_commit(&repo.path, branch, target_branch)
        {
            Ok(commit) => commit,
            Err(_) => {
                summary_failed = true;
                continue;
            }
        };

        match deployment
            .git()
            .get_worktree_diff_summary(&worktree_path, &base_commit, None)
        {
            Ok(repo_summary) => {
                summary.file_count = summary.file_count.saturating_add(repo_summary.file_count);
                summary.added = summary.added.saturating_add(repo_summary.added);
                summary.deleted = summary.deleted.saturating_add(repo_summary.deleted);
                summary.total_bytes = summary.total_bytes.saturating_add(repo_summary.total_bytes);
            }
            Err(_) => {
                summary_failed = true;
            }
        }

        repo_inputs.push((repo, worktree_path, base_commit));
    }

    let guard_enabled = diff_stream::diff_preview_guard_thresholds(guard_preset.clone()).is_some();
    let blocked_by_guard = !request.force
        && guard_enabled
        && (summary_failed || diff_stream::diff_preview_guard_exceeded(&summary, guard_preset));
    if blocked_by_guard {
        let reason = if summary_failed {
            AttemptArtifactBlockedReason::SummaryFailed
        } else {
            AttemptArtifactBlockedReason::ThresholdExceeded
        };
        return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
            blocked: true,
            blocked_reason: Some(reason),
            truncated: false,
            bytes: 0,
            paths: request.paths,
            patch: None,
        })));
    }

    let mut requested_by_repo: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for raw in &request.paths {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((repo_name, rel)) = trimmed.split_once('/') else {
            return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
                blocked: true,
                blocked_reason: Some(AttemptArtifactBlockedReason::PathOutsideWorkspace),
                truncated: false,
                bytes: 0,
                paths: request.paths,
                patch: None,
            })));
        };
        let rel = rel.trim();
        if rel.is_empty() {
            continue;
        }
        let rel_path = PathBuf::from(rel);
        let invalid = rel_path.is_absolute()
            || rel_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir));
        if invalid {
            return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
                blocked: true,
                blocked_reason: Some(AttemptArtifactBlockedReason::PathOutsideWorkspace),
                truncated: false,
                bytes: 0,
                paths: request.paths,
                patch: None,
            })));
        }
        requested_by_repo
            .entry(repo_name.to_string())
            .or_default()
            .push(rel.to_string());
    }

    let mut patch = String::new();
    for (repo, worktree_path, base_commit) in &repo_inputs {
        let Some(rel_paths) = requested_by_repo.get(&repo.name) else {
            continue;
        };

        let filter: Vec<&str> = rel_paths.iter().map(|s| s.as_str()).collect();
        let diffs = match deployment.git().get_diffs(
            DiffTarget::Worktree {
                worktree_path,
                base_commit,
            },
            Some(&filter),
            DiffContentPolicy::Full,
        ) {
            Ok(diffs) => diffs,
            Err(_) => {
                return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
                    blocked: true,
                    blocked_reason: Some(AttemptArtifactBlockedReason::SummaryFailed),
                    truncated: false,
                    bytes: 0,
                    paths: request.paths,
                    patch: None,
                })));
            }
        };

        for diff in diffs {
            let Some(path) = diff.new_path.or(diff.old_path) else {
                continue;
            };

            let old = diff.old_content.unwrap_or_default();
            let new = diff.new_content.unwrap_or_default();
            let file_path = format!("{}/{}", repo.name, path);
            patch.push_str(&create_unified_diff(&file_path, &old, &new));
            if !patch.ends_with('\n') {
                patch.push('\n');
            }
        }
    }

    if patch.is_empty() {
        return Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
            blocked: false,
            blocked_reason: None,
            truncated: false,
            bytes: 0,
            paths: request.paths,
            patch: Some(String::new()),
        })));
    }

    let truncated = patch.len() > max_bytes;
    let patch = if truncated {
        truncate_to_char_boundary(&patch, max_bytes).to_string()
    } else {
        patch
    };
    let bytes = patch.as_bytes().len();

    Ok(ResponseJson(ApiResponse::success(AttemptPatchResponse {
        blocked: false,
        blocked_reason: None,
        truncated,
        bytes,
        paths: request.paths,
        patch: Some(patch),
    })))
}

fn map_task_group_error(err: TaskGroupError) -> ApiError {
    match err {
        TaskGroupError::Database(db_err) => ApiError::Database(db_err),
        _ => ApiError::BadRequest(err.to_string()),
    }
}

fn map_workspace_error(err: WorkspaceError) -> ApiError {
    match err {
        WorkspaceError::Database(db_err) => ApiError::Database(db_err),
        _ => ApiError::BadRequest(err.to_string()),
    }
}

fn find_task_group_node<'a>(
    graph: &'a TaskGroupGraph,
    node_id: &str,
) -> Result<&'a TaskGroupNode, ApiError> {
    let node_key = node_id.trim();
    if node_key.is_empty() {
        return Err(ApiError::BadRequest(
            "Task group node id cannot be empty".to_string(),
        ));
    }

    graph
        .nodes
        .iter()
        .find(|node| node.id.trim() == node_key)
        .ok_or_else(|| ApiError::BadRequest("Task group node not found in graph".to_string()))
}

fn resolve_executor_profile_id(
    task_group_node: &TaskGroupNode,
    fallback: ExecutorProfileId,
) -> ExecutorProfileId {
    task_group_node
        .executor_profile_id
        .clone()
        .unwrap_or(fallback)
}

async fn resolve_topology_base_branches(
    pool: &DbPool,
    graph: &TaskGroupGraph,
    node_id: &str,
) -> Result<Option<HashMap<Uuid, String>>, ApiError> {
    let node_key = node_id.trim();
    if node_key.is_empty() {
        return Err(ApiError::BadRequest(
            "Task group node id cannot be empty".to_string(),
        ));
    }

    let mut selected_workspace: Option<(Uuid, chrono::DateTime<chrono::Utc>)> = None;

    for edge in &graph.edges {
        if edge.to.trim() != node_key {
            continue;
        }
        let from = edge.from.trim();
        if from.is_empty() {
            continue;
        }
        let predecessor = graph.nodes.iter().find(|node| node.id.trim() == from);
        let Some(predecessor) = predecessor else {
            continue;
        };
        if predecessor.status.clone().unwrap_or(TaskStatus::Todo) != TaskStatus::Done {
            continue;
        }

        let workspaces = Workspace::fetch_all(pool, Some(predecessor.task_id))
            .await
            .map_err(map_workspace_error)?;
        let Some(workspace) = workspaces.first() else {
            continue;
        };

        let created_at = workspace.created_at;
        let replace = selected_workspace
            .as_ref()
            .map(|(existing_id, existing_at)| {
                if created_at == *existing_at {
                    workspace.id < *existing_id
                } else {
                    created_at > *existing_at
                }
            })
            .unwrap_or(true);
        if replace {
            selected_workspace = Some((workspace.id, created_at));
        }
    }

    let Some((workspace_id, _)) = selected_workspace else {
        return Ok(None);
    };

    let repos = WorkspaceRepo::find_by_workspace_id(pool, workspace_id)
        .await
        .map_err(ApiError::Database)?;
    if repos.is_empty() {
        return Ok(None);
    }

    let mut base_branches = HashMap::with_capacity(repos.len());
    for repo in repos {
        base_branches.insert(repo.repo_id, repo.target_branch);
    }

    Ok(Some(base_branches))
}

fn blocked_predecessors(graph: &TaskGroupGraph, node_id: &str) -> Result<Vec<String>, ApiError> {
    let node_key = node_id.trim();
    if node_key.is_empty() {
        return Err(ApiError::BadRequest(
            "Task group node id cannot be empty".to_string(),
        ));
    }

    let node_statuses: HashMap<String, TaskStatus> = graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.trim().to_string(),
                node.status.clone().unwrap_or(TaskStatus::Todo),
            )
        })
        .collect();

    if !node_statuses.contains_key(node_key) {
        return Err(ApiError::BadRequest(
            "Task group node not found in graph".to_string(),
        ));
    }

    let mut blocked = Vec::new();
    for edge in &graph.edges {
        if edge.to.trim() != node_key {
            continue;
        }
        let from = edge.from.trim();
        if from.is_empty() {
            continue;
        }
        match node_statuses.get(from) {
            Some(status) if *status != TaskStatus::Done => blocked.push(from.to_string()),
            None => blocked.push(from.to_string()),
            _ => {}
        }
    }

    Ok(blocked)
}

#[axum::debug_handler]
pub async fn create_task_attempt(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    Json(payload): Json<CreateTaskAttemptBody>,
) -> Result<ResponseJson<ApiResponse<Workspace>>, ApiError> {
    if payload.repos.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one repository is required".to_string(),
        ));
    }

    let key = crate::routes::idempotency::idempotency_key(&headers);
    let hash = crate::routes::idempotency::request_hash(&payload)?;

    crate::routes::idempotency::idempotent_success(
        &deployment.db().pool,
        "create_task_attempt",
        key,
        hash,
        || async {
            let mut executor_profile_id = payload.executor_profile_id.clone();

            let pool = &deployment.db().pool;
            let task = Task::find_by_id(&deployment.db().pool, payload.task_id)
                .await?
                .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
            let original_task_status = task.status.clone();
            let mut baseline_ref: Option<String> = None;
            let mut topology_branches: Option<HashMap<Uuid, String>> = None;
            if let (Some(task_group_id), Some(node_id)) =
                (task.task_group_id, task.task_group_node_id.as_ref())
            {
                let task_group = TaskGroup::find_by_id(pool, task_group_id)
                    .await
                    .map_err(map_task_group_error)?
                    .ok_or_else(|| ApiError::BadRequest("Task group not found".to_string()))?;

                let blocked = blocked_predecessors(&task_group.graph, node_id)?;
                if !blocked.is_empty() {
                    return Err(ApiError::Conflict(format!(
                        "Task is blocked by incomplete predecessors: {}",
                        blocked.join(", ")
                    )));
                }

                let task_group_node = find_task_group_node(&task_group.graph, node_id)?;
                executor_profile_id =
                    resolve_executor_profile_id(task_group_node, executor_profile_id);

                match &task_group_node.base_strategy {
                    TaskGroupNodeBaseStrategy::Baseline => {
                        let trimmed = task_group.baseline_ref.trim();
                        if !trimmed.is_empty() {
                            baseline_ref = Some(trimmed.to_string());
                        }
                    }
                    TaskGroupNodeBaseStrategy::Topology => {
                        topology_branches =
                            resolve_topology_base_branches(pool, &task_group.graph, node_id)
                                .await?;
                        if topology_branches.is_none() {
                            let trimmed = task_group.baseline_ref.trim();
                            if !trimmed.is_empty() {
                                baseline_ref = Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }

            let project = task
                .parent_project(pool)
                .await?
                .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

            let agent_working_dir = project
                .default_agent_working_dir
                .as_ref()
                .filter(|dir| !dir.is_empty())
                .cloned();

            let attempt_id = Uuid::new_v4();
            let git_branch_name = deployment
                .container()
                .git_branch_from_workspace(&attempt_id, &task.title)
                .await;

            let tx = pool.begin().await?;
            let workspace = Workspace::create(
                &tx,
                &CreateWorkspace {
                    branch: git_branch_name.clone(),
                    agent_working_dir,
                },
                attempt_id,
                payload.task_id,
            )
            .await?;

            let workspace_repos: Vec<CreateWorkspaceRepo> = payload
                .repos
                .iter()
                .map(|r| {
                    let target_branch = topology_branches
                        .as_ref()
                        .and_then(|branches| branches.get(&r.repo_id))
                        .cloned()
                        .or_else(|| baseline_ref.clone())
                        .unwrap_or_else(|| r.target_branch.clone());
                    CreateWorkspaceRepo {
                        repo_id: r.repo_id,
                        target_branch,
                    }
                })
                .collect();

            WorkspaceRepo::create_many(&tx, workspace.id, &workspace_repos).await?;
            tx.commit().await?;

            if let Err(err) = deployment
                .container()
                .start_workspace(&workspace, executor_profile_id.clone())
                .await
            {
                tracing::error!(
                    task_id = %task.id,
                    workspace_id = %workspace.id,
                    error = %err,
                    "Failed to start task attempt"
                );
                if let Err(cleanup_err) = cleanup_failed_attempt_start(
                    &deployment,
                    &task,
                    &workspace,
                    &original_task_status,
                )
                .await
                {
                    tracing::error!(
                        task_id = %task.id,
                        workspace_id = %workspace.id,
                        error = %cleanup_err,
                        "Failed to cleanup attempt after start failure"
                    );
                }
                return Err(ApiError::from(err));
            }

            tracing::info!(
                "Created and started attempt {} for task {}",
                workspace.id,
                task.id
            );
            Ok(workspace)
        },
    )
    .await
}

async fn cleanup_failed_attempt_start(
    deployment: &DeploymentImpl,
    task: &Task,
    workspace: &Workspace,
    original_task_status: &TaskStatus,
) -> Result<(), ApiError> {
    let pool = &deployment.db().pool;
    let workspace_for_cleanup = Workspace::find_by_id(pool, workspace.id)
        .await?
        .unwrap_or_else(|| workspace.clone());

    if let Err(err) = deployment.container().delete(&workspace_for_cleanup).await {
        tracing::error!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            error = %err,
            "Failed to delete workspace worktree after start failure"
        );
    }

    if let Ok(Some(current_task)) = Task::find_by_id(pool, task.id).await
        && current_task.status != *original_task_status
        && matches!(
            current_task.status,
            TaskStatus::InProgress | TaskStatus::InReview
        )
    {
        match Task::has_running_attempts(pool, task.id).await {
            Ok(false) => {
                let should_restore = match Task::latest_attempt_workspace_id(pool, task.id).await {
                    Ok(Some(latest_workspace_id)) => latest_workspace_id == workspace.id,
                    Ok(None) => true,
                    Err(err) => {
                        tracing::error!(
                            task_id = %task.id,
                            workspace_id = %workspace.id,
                            error = %err,
                            "Failed to resolve latest attempt workspace after start failure"
                        );
                        false
                    }
                };

                if should_restore
                    && let Err(err) =
                        Task::update_status(pool, task.id, original_task_status.clone()).await
                {
                    tracing::error!(
                        task_id = %task.id,
                        workspace_id = %workspace.id,
                        error = %err,
                        "Failed to restore task status after start failure"
                    );
                }
            }
            Ok(true) => {}
            Err(err) => {
                tracing::error!(
                    task_id = %task.id,
                    workspace_id = %workspace.id,
                    error = %err,
                    "Failed to check running attempts after start failure"
                );
            }
        }
    }

    let rows = Workspace::delete(pool, workspace.id).await?;
    if rows == 0 {
        tracing::warn!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            "Workspace cleanup skipped because workspace no longer exists"
        );
    }

    Ok(())
}

#[axum::debug_handler]
pub async fn run_agent_setup(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RunAgentSetupRequest>,
) -> Result<ResponseJson<ApiResponse<RunAgentSetupResponse>>, ApiError> {
    let executor_profile_id = payload.executor_profile_id;
    let config = ExecutorConfigs::get_cached();
    let coding_agent = config.get_coding_agent_or_default(&executor_profile_id);
    match coding_agent {
        CodingAgent::CursorAgent(_) => {
            cursor_setup::run_cursor_setup(&deployment, &workspace).await?;
        }
        CodingAgent::Codex(codex) => {
            codex_setup::run_codex_setup(&deployment, &workspace, &codex).await?;
        }
        _ => return Err(ApiError::Executor(ExecutorError::SetupHelperNotSupported)),
    }

    Ok(ResponseJson(ApiResponse::success(RunAgentSetupResponse {})))
}

#[axum::debug_handler]
pub async fn merge_task_attempt(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<MergeTaskAttemptRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(repo.name);

    let task = workspace
        .parent_task(pool)
        .await?
        .ok_or(ApiError::Workspace(WorkspaceError::TaskNotFound))?;
    let task_uuid_str = task.id.to_string();
    let first_uuid_section = task_uuid_str.split('-').next().unwrap_or(&task_uuid_str);

    let mut commit_message = format!("{} (vibe-kanban {})", task.title, first_uuid_section);

    // Add description on next line if it exists
    if let Some(description) = &task.description
        && !description.trim().is_empty()
    {
        commit_message.push_str("\n\n");
        commit_message.push_str(description);
    }

    let no_verify = deployment.config().read().await.git_no_verify;
    let git = deployment.git().clone();
    let repo_path = repo.path.clone();
    let workspace_branch = workspace.branch.clone();
    let target_branch = workspace_repo.target_branch.clone();
    let merge_commit_id = run_git_operation(git, move |git| {
        git.merge_changes_with_options(
            &repo_path,
            &worktree_path,
            &workspace_branch,
            &target_branch,
            &commit_message,
            GitMergeOptions::new(no_verify),
        )
    })
    .await?;

    Merge::create_direct(
        pool,
        workspace.id,
        workspace_repo.repo_id,
        &workspace_repo.target_branch,
        &merge_commit_id,
    )
    .await?;
    Task::update_status(pool, task.id, TaskStatus::Done).await?;

    // Stop any running dev servers for this workspace
    let dev_servers =
        ExecutionProcess::find_running_dev_servers_by_workspace(pool, workspace.id).await?;

    for dev_server in dev_servers {
        tracing::info!(
            "Stopping dev server {} for completed task attempt {}",
            dev_server.id,
            workspace.id
        );

        if let Err(e) = deployment
            .container()
            .stop_execution(&dev_server, ExecutionProcessStatus::Killed)
            .await
        {
            tracing::error!(
                "Failed to stop dev server {} for task attempt {}: {}",
                dev_server.id,
                workspace.id,
                e
            );
        }
    }

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn push_task_attempt_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<PushTaskAttemptRequest>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<(), PushError>>), ApiError> {
    let pool = &deployment.db().pool;

    let github_service = GitHubService::new()?;
    github_service.check_token().await?;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let git = deployment.git().clone();
    let workspace_branch = workspace.branch.clone();
    let push_result = run_git_operation(git, move |git| {
        git.push_to_github(&worktree_path, &workspace_branch, false)
    })
    .await;

    match push_result {
        Ok(_) => Ok((StatusCode::OK, ResponseJson(ApiResponse::success(())))),
        Err(GitServiceError::GitCLI(GitCliError::PushRejected(_))) => Ok((
            StatusCode::CONFLICT,
            ResponseJson(ApiResponse::error_with_data(PushError::ForcePushRequired)),
        )),
        Err(e) => Err(ApiError::GitService(e)),
    }
}

pub async fn force_push_task_attempt_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<PushTaskAttemptRequest>,
) -> Result<ResponseJson<ApiResponse<(), PushError>>, ApiError> {
    let pool = &deployment.db().pool;

    let github_service = GitHubService::new()?;
    github_service.check_token().await?;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let git = deployment.git().clone();
    let workspace_branch = workspace.branch.clone();
    run_git_operation(git, move |git| {
        git.push_to_github(&worktree_path, &workspace_branch, true)
    })
    .await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn open_task_attempt_in_editor(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<OpenEditorRequest>,
) -> Result<ResponseJson<ApiResponse<OpenEditorResponse>>, ApiError> {
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);

    // For single-repo projects, open from the repo directory
    let workspace_repos =
        WorkspaceRepo::find_repos_for_workspace(&deployment.db().pool, workspace.id).await?;
    let workspace_path = if workspace_repos.len() == 1 && payload.file_path.is_none() {
        workspace_path.join(&workspace_repos[0].name)
    } else {
        workspace_path.to_path_buf()
    };

    // If a specific file path is provided, use it; otherwise use the base path
    let path = if let Some(file_path) = payload.file_path.as_ref() {
        workspace_path.join(file_path)
    } else {
        workspace_path
    };

    let editor_config = {
        let config = deployment.config().read().await;
        let editor_type_str = payload.editor_type.as_deref();
        config.editor.with_override(editor_type_str)
    };

    match editor_config.open_file(path.as_path()).await {
        Ok(url) => {
            tracing::info!(
                "Opened editor for task attempt {} at path: {}{}",
                workspace.id,
                path.display(),
                if url.is_some() { " (remote mode)" } else { "" }
            );

            Ok(ResponseJson(ApiResponse::success(OpenEditorResponse {
                url,
            })))
        }
        Err(e) => {
            tracing::error!(
                "Failed to open editor for attempt {}: {:?}",
                workspace.id,
                e
            );
            Err(ApiError::EditorOpen(e))
        }
    }
}

pub async fn get_task_attempt_branch_status(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<RepoBranchStatus>>>, ApiError> {
    let pool = &deployment.db().pool;

    let repositories = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let workspace_repos = WorkspaceRepo::find_by_workspace_id(pool, workspace.id).await?;
    let target_branches: HashMap<_, _> = workspace_repos
        .iter()
        .map(|wr| (wr.repo_id, wr.target_branch.clone()))
        .collect();

    let workspace_dir = match workspace
        .container_ref
        .as_ref()
        .map(PathBuf::from)
        .filter(|path| path.exists())
    {
        Some(path) => path,
        None => match deployment
            .container()
            .ensure_container_exists(&workspace)
            .await
        {
            Ok(container_ref) => PathBuf::from(container_ref),
            Err(err) => {
                tracing::warn!(
                    "Failed to ensure workspace container for branch status {}: {}",
                    workspace.id,
                    err
                );
                return Ok(ResponseJson(ApiResponse::success(Vec::new())));
            }
        },
    };

    let mut results = Vec::with_capacity(repositories.len());

    for repo in repositories {
        let Some(target_branch) = target_branches.get(&repo.id).cloned() else {
            continue;
        };

        let repo_merges = Merge::find_by_workspace_and_repo_id(pool, workspace.id, repo.id).await?;

        let worktree_path = workspace_dir.join(&repo.name);
        let git = deployment.git().clone();
        let repo_name = repo.name.clone();
        let repo_path = repo.path.clone();
        let workspace_branch = workspace.branch.clone();
        let target_branch_for_git = target_branch.clone();
        let has_open_pr = matches!(
            repo_merges.first(),
            Some(Merge::Pr(PrMerge {
                pr_info: PullRequestInfo {
                    status: MergeStatus::Open,
                    ..
                },
                ..
            }))
        );
        let (
            head_oid,
            is_rebase_in_progress,
            conflicted_files,
            conflict_op,
            commits_ahead,
            commits_behind,
            uncommitted_count,
            untracked_count,
            remote_ahead,
            remote_behind,
        ) = run_git_operation(git, move |git| {
            let head_oid = git.get_head_info(&worktree_path).ok().map(|h| h.oid);

            let is_rebase_in_progress = git.is_rebase_in_progress(&worktree_path).unwrap_or(false);
            let conflicted_files = git.get_conflicted_files(&worktree_path).unwrap_or_default();
            let conflict_op = if conflicted_files.is_empty() {
                None
            } else {
                git.detect_conflict_op(&worktree_path).unwrap_or(None)
            };

            let (uncommitted_count, untracked_count) =
                match git.get_worktree_change_counts(&worktree_path) {
                    Ok((a, b)) => (Some(a), Some(b)),
                    Err(_) => (None, None),
                };

            let target_branch_type = match git.find_branch_type(&repo_path, &target_branch_for_git)
            {
                Ok(branch_type) => Some(branch_type),
                Err(err) => {
                    tracing::debug!(
                        "Failed to detect branch type for repo {}: {}",
                        repo_name,
                        err
                    );
                    None
                }
            };

            let (commits_ahead, commits_behind) = match target_branch_type {
                Some(BranchType::Local) => {
                    match git.get_branch_status(
                        &repo_path,
                        &workspace_branch,
                        &target_branch_for_git,
                    ) {
                        Ok((a, b)) => (Some(a), Some(b)),
                        Err(err) => {
                            tracing::debug!(
                                "Failed to get local branch status for repo {}: {}",
                                repo_name,
                                err
                            );
                            (None, None)
                        }
                    }
                }
                Some(BranchType::Remote) => {
                    match git.get_remote_branch_status(
                        &repo_path,
                        &workspace_branch,
                        Some(&target_branch_for_git),
                    ) {
                        Ok((ahead, behind)) => (Some(ahead), Some(behind)),
                        Err(err) => {
                            tracing::debug!(
                                "Failed to get remote branch status for repo {}: {}",
                                repo_name,
                                err
                            );
                            (None, None)
                        }
                    }
                }
                None => (None, None),
            };

            let (remote_ahead, remote_behind) = if has_open_pr {
                match git.get_remote_branch_status(&repo_path, &workspace_branch, None) {
                    Ok((ahead, behind)) => (Some(ahead), Some(behind)),
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            };

            Ok((
                head_oid,
                is_rebase_in_progress,
                conflicted_files,
                conflict_op,
                commits_ahead,
                commits_behind,
                uncommitted_count,
                untracked_count,
                remote_ahead,
                remote_behind,
            ))
        })
        .await?;

        let has_uncommitted_changes = uncommitted_count.map(|c| c > 0);

        results.push(RepoBranchStatus {
            repo_id: repo.id,
            repo_name: repo.name,
            status: BranchStatus {
                commits_ahead,
                commits_behind,
                has_uncommitted_changes,
                head_oid,
                uncommitted_count,
                untracked_count,
                remote_commits_ahead: remote_ahead,
                remote_commits_behind: remote_behind,
                merges: repo_merges,
                target_branch_name: target_branch,
                is_rebase_in_progress,
                conflict_op,
                conflicted_files,
            },
        });
    }

    Ok(ResponseJson(ApiResponse::success(results)))
}

#[axum::debug_handler]
pub async fn change_target_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<ChangeTargetBranchRequest>,
) -> Result<ResponseJson<ApiResponse<ChangeTargetBranchResponse>>, ApiError> {
    let repo_id = payload.repo_id;
    let new_target_branch = payload.new_target_branch;
    let pool = &deployment.db().pool;

    let repo = Repo::find_by_id(pool, repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let git = deployment.git().clone();
    let repo_path = repo.path.clone();
    let branch_to_check = new_target_branch.clone();
    if !run_git_operation(git, move |git| {
        git.check_branch_exists(&repo_path, &branch_to_check)
    })
    .await?
    {
        return Err(ApiError::BadRequest(format!(
            "Branch '{}' does not exist in repository '{}'",
            new_target_branch, repo.name
        )));
    };

    WorkspaceRepo::update_target_branch(pool, workspace.id, repo_id, &new_target_branch).await?;

    let git = deployment.git().clone();
    let repo_path = repo.path.clone();
    let workspace_branch = workspace.branch.clone();
    let target_branch = new_target_branch.clone();
    let status = run_git_operation(git, move |git| {
        git.get_branch_status(&repo_path, &workspace_branch, &target_branch)
    })
    .await?;

    Ok(ResponseJson(ApiResponse::success(
        ChangeTargetBranchResponse {
            repo_id,
            new_target_branch,
            status,
        },
    )))
}

#[axum::debug_handler]
pub async fn rename_branch(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RenameBranchRequest>,
) -> Result<
    (
        StatusCode,
        ResponseJson<ApiResponse<RenameBranchResponse, RenameBranchError>>,
    ),
    ApiError,
> {
    let new_branch_name = payload.new_branch_name.trim();

    if new_branch_name.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            ResponseJson(ApiResponse::error_with_data(
                RenameBranchError::EmptyBranchName,
            )),
        ));
    }
    if !deployment.git().is_branch_name_valid(new_branch_name) {
        return Ok((
            StatusCode::BAD_REQUEST,
            ResponseJson(ApiResponse::error_with_data(
                RenameBranchError::InvalidBranchNameFormat,
            )),
        ));
    }
    if new_branch_name == workspace.branch {
        return Ok((
            StatusCode::OK,
            ResponseJson(ApiResponse::success(RenameBranchResponse {
                branch: workspace.branch.clone(),
            })),
        ));
    }

    let pool = &deployment.db().pool;

    // Fail if workspace has an open PR in any repo
    let merges = Merge::find_by_workspace_id(pool, workspace.id).await?;
    let has_open_pr = merges.into_iter().any(|merge| {
        matches!(merge, Merge::Pr(pr_merge) if matches!(pr_merge.pr_info.status, MergeStatus::Open))
    });
    if has_open_pr {
        return Ok((
            StatusCode::CONFLICT,
            ResponseJson(ApiResponse::error_with_data(
                RenameBranchError::OpenPullRequest,
            )),
        ));
    }

    let repos = WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?;
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_dir = PathBuf::from(&container_ref);

    for repo in &repos {
        let worktree_path = workspace_dir.join(&repo.name);
        let git = deployment.git().clone();
        let repo_path = repo.path.clone();
        let branch_name = new_branch_name.to_string();
        let branch_exists = run_git_operation(git, move |git| {
            git.check_branch_exists(&repo_path, &branch_name)
        })
        .await?;
        if branch_exists {
            return Ok((
                StatusCode::CONFLICT,
                ResponseJson(ApiResponse::error_with_data(
                    RenameBranchError::BranchAlreadyExists {
                        repo_name: repo.name.clone(),
                    },
                )),
            ));
        }

        let git = deployment.git().clone();
        let is_rebase_in_progress =
            run_git_operation(git, move |git| git.is_rebase_in_progress(&worktree_path)).await?;
        if is_rebase_in_progress {
            return Ok((
                StatusCode::CONFLICT,
                ResponseJson(ApiResponse::error_with_data(
                    RenameBranchError::RebaseInProgress {
                        repo_name: repo.name.clone(),
                    },
                )),
            ));
        }
    }

    // Rename all repos with rollback
    let old_branch = workspace.branch.clone();
    let mut renamed_repos: Vec<&Repo> = Vec::new();

    for repo in &repos {
        let worktree_path = workspace_dir.join(&repo.name);
        let git = deployment.git().clone();
        let old_name = workspace.branch.clone();
        let new_name = new_branch_name.to_string();
        match run_git_operation(git, move |git| {
            git.rename_local_branch(&worktree_path, &old_name, &new_name)
        })
        .await
        {
            Ok(()) => {
                renamed_repos.push(repo);
            }
            Err(e) => {
                // Rollback already renamed repos
                for renamed_repo in &renamed_repos {
                    let rollback_path = workspace_dir.join(&renamed_repo.name);
                    let git = deployment.git().clone();
                    let new_name = new_branch_name.to_string();
                    let old_name = old_branch.clone();
                    if let Err(rollback_err) = run_git_operation(git, move |git| {
                        git.rename_local_branch(&rollback_path, &new_name, &old_name)
                    })
                    .await
                    {
                        tracing::error!(
                            "Failed to rollback branch rename in '{}': {}",
                            renamed_repo.name,
                            rollback_err
                        );
                    }
                }
                return Ok((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson(ApiResponse::error_with_data(
                        RenameBranchError::RenameFailed {
                            repo_name: repo.name.clone(),
                            message: e.to_string(),
                        },
                    )),
                ));
            }
        }
    }

    Workspace::update_branch_name(pool, workspace.id, new_branch_name).await?;
    // What will become of me?
    let updated_children_count = WorkspaceRepo::update_target_branch_for_children_of_workspace(
        pool,
        workspace.id,
        &old_branch,
        new_branch_name,
    )
    .await?;

    if updated_children_count > 0 {
        tracing::info!(
            "Updated {} child task attempts to target new branch '{}'",
            updated_children_count,
            new_branch_name
        );
    }

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(RenameBranchResponse {
            branch: new_branch_name.to_string(),
        })),
    ))
}

#[axum::debug_handler]
pub async fn rebase_task_attempt(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<RebaseTaskAttemptRequest>,
) -> Result<ResponseJson<ApiResponse<(), GitOperationError>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, payload.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let old_base_branch = payload
        .old_base_branch
        .unwrap_or_else(|| workspace_repo.target_branch.clone());
    let new_base_branch = payload
        .new_base_branch
        .unwrap_or_else(|| workspace_repo.target_branch.clone());

    let git = deployment.git().clone();
    let repo_path = repo.path.clone();
    let target_branch = new_base_branch.clone();
    match run_git_operation(git, move |git| {
        git.check_branch_exists(&repo_path, &target_branch)
    })
    .await?
    {
        true => {
            WorkspaceRepo::update_target_branch(
                pool,
                workspace.id,
                payload.repo_id,
                &new_base_branch,
            )
            .await?;
        }
        false => {
            return Err(ApiError::BadRequest(format!(
                "Branch '{}' does not exist in the repository",
                new_base_branch
            )));
        }
    }

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let git = deployment.git().clone();
    let repo_path = repo.path.clone();
    let workspace_branch = workspace.branch.clone();
    let result = run_git_operation(git, move |git| {
        git.rebase_branch(
            &repo_path,
            &worktree_path,
            &new_base_branch,
            &old_base_branch,
            &workspace_branch,
        )
    })
    .await;
    if let Err(e) = result {
        use services::services::git::GitServiceError;
        return match e {
            GitServiceError::MergeConflicts(msg) => Ok(ResponseJson(ApiResponse::<
                (),
                GitOperationError,
            >::error_with_data(
                GitOperationError::MergeConflicts {
                    message: msg,
                    op: ConflictOp::Rebase,
                },
            ))),
            GitServiceError::RebaseInProgress => Ok(ResponseJson(ApiResponse::<
                (),
                GitOperationError,
            >::error_with_data(
                GitOperationError::RebaseInProgress,
            ))),
            other => Err(ApiError::GitService(other)),
        };
    }

    Ok(ResponseJson(ApiResponse::success(())))
}

#[axum::debug_handler]
pub async fn abort_conflicts_task_attempt(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<AbortConflictsRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    let repo = Repo::find_by_id(pool, payload.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = Path::new(&container_ref);
    let worktree_path = workspace_path.join(&repo.name);

    let git = deployment.git().clone();
    run_git_operation(git, move |git| git.abort_conflicts(&worktree_path)).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

#[axum::debug_handler]
pub async fn start_dev_server(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    // Get parent task
    let task = workspace
        .parent_task(&deployment.db().pool)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

    // Get parent project
    let project = task
        .parent_project(&deployment.db().pool)
        .await?
        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

    // Stop any existing dev servers for this project
    let existing_dev_servers =
        match ExecutionProcess::find_running_dev_servers_by_project(pool, project.id).await {
            Ok(servers) => servers,
            Err(e) => {
                tracing::error!(
                    "Failed to find running dev servers for project {}: {}",
                    project.id,
                    e
                );
                return Err(ApiError::Workspace(WorkspaceError::ValidationError(
                    e.to_string(),
                )));
            }
        };

    for dev_server in existing_dev_servers {
        tracing::info!(
            "Stopping existing dev server {} for project {}",
            dev_server.id,
            project.id
        );

        if let Err(e) = deployment
            .container()
            .stop_execution(&dev_server, ExecutionProcessStatus::Killed)
            .await
        {
            tracing::error!("Failed to stop dev server {}: {}", dev_server.id, e);
        }
    }

    // Get dev script from project (dev_script is project-level, not per-repo)
    let dev_script = match &project.dev_script {
        Some(script) if !script.is_empty() => script.clone(),
        _ => {
            return Err(ApiError::BadRequest(
                "No dev server script configured for this project".to_string(),
            ));
        }
    };
    validate_dev_server_script(&dev_script)?;
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_root = PathBuf::from(&container_ref);
    let working_dir = normalize_dev_server_working_dir(
        &workspace_root,
        project.dev_script_working_dir.as_deref(),
    )?;

    tracing::info!(
        project_id = %project.id,
        workspace_id = %workspace.id,
        has_working_dir = %working_dir.is_some(),
        "Audit: starting dev server script execution"
    );

    let executor_action = ExecutorAction::new(
        ExecutorActionType::ScriptRequest(ScriptRequest {
            script: dev_script,
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::DevServer,
            working_dir,
        }),
        None,
    );

    // Get or create a session for dev server
    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: Some("dev-server".to_string()),
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::DevServer,
        )
        .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn get_task_attempt_children(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TaskRelationships>>, ApiError> {
    let relationships =
        Task::find_relationships_for_workspace(&deployment.db().pool, &workspace).await?;
    Ok(ResponseJson(ApiResponse::success(relationships)))
}

pub async fn stop_task_attempt_execution(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<StopTaskAttemptQuery>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    if query.force.unwrap_or(false) {
        deployment
            .container()
            .try_stop_force(&workspace, false)
            .await;
    } else {
        deployment.container().try_stop(&workspace, false).await;
    }

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn remove_task_attempt_worktree(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    if ExecutionProcess::has_running_non_dev_server_processes_for_workspace(pool, workspace.id)
        .await?
    {
        return Err(ApiError::Conflict(
            "Attempt has running processes. Stop them before removing the worktree.".to_string(),
        ));
    }

    if !ExecutionProcess::find_running_dev_servers_by_workspace(pool, workspace.id)
        .await?
        .is_empty()
    {
        return Err(ApiError::Conflict(
            "Attempt has a running dev server. Stop it before removing the worktree.".to_string(),
        ));
    }

    if workspace.container_ref.is_none() {
        return Ok(ResponseJson(ApiResponse::success(())));
    }

    deployment.container().delete(&workspace).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

#[axum::debug_handler]
pub async fn run_setup_script(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<
    (
        StatusCode,
        ResponseJson<ApiResponse<ExecutionProcess, RunScriptError>>,
    ),
    ApiError,
> {
    let pool = &deployment.db().pool;

    // Check if any non-dev-server processes are already running for this workspace
    if ExecutionProcess::has_running_non_dev_server_processes_for_workspace(pool, workspace.id)
        .await?
    {
        return Ok((
            StatusCode::CONFLICT,
            ResponseJson(ApiResponse::error_with_data(
                RunScriptError::ProcessAlreadyRunning,
            )),
        ));
    }

    deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;

    // Get parent task and project
    let task = workspace
        .parent_task(pool)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

    let project = task
        .parent_project(pool)
        .await?
        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
    let project_repos = ProjectRepo::find_by_project_id_with_names(pool, project.id).await?;
    let executor_action = match deployment
        .container()
        .setup_actions_for_repos(&project_repos)
    {
        Some(action) => action,
        None => {
            return Ok((
                StatusCode::BAD_REQUEST,
                ResponseJson(ApiResponse::error_with_data(
                    RunScriptError::NoScriptConfigured,
                )),
            ));
        }
    };

    // Get or create a session for setup script
    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: Some("setup-script".to_string()),
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let execution_process = deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::SetupScript,
        )
        .await?;

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(execution_process)),
    ))
}

#[axum::debug_handler]
pub async fn run_cleanup_script(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<
    (
        StatusCode,
        ResponseJson<ApiResponse<ExecutionProcess, RunScriptError>>,
    ),
    ApiError,
> {
    let pool = &deployment.db().pool;

    // Check if any non-dev-server processes are already running for this workspace
    if ExecutionProcess::has_running_non_dev_server_processes_for_workspace(pool, workspace.id)
        .await?
    {
        return Ok((
            StatusCode::CONFLICT,
            ResponseJson(ApiResponse::error_with_data(
                RunScriptError::ProcessAlreadyRunning,
            )),
        ));
    }

    deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;

    // Get parent task and project
    let task = workspace
        .parent_task(pool)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;

    let project = task
        .parent_project(pool)
        .await?
        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
    let project_repos = ProjectRepo::find_by_project_id_with_names(pool, project.id).await?;
    let executor_action = match deployment
        .container()
        .cleanup_actions_for_repos(&project_repos)
    {
        Some(action) => action,
        None => {
            return Ok((
                StatusCode::BAD_REQUEST,
                ResponseJson(ApiResponse::error_with_data(
                    RunScriptError::NoScriptConfigured,
                )),
            ));
        }
    };

    // Get or create a session for cleanup script
    let session = match Session::find_latest_by_workspace_id(pool, workspace.id).await? {
        Some(s) => s,
        None => {
            Session::create(
                pool,
                &CreateSession {
                    executor: Some("cleanup-script".to_string()),
                },
                Uuid::new_v4(),
                workspace.id,
            )
            .await?
        }
    };

    let execution_process = deployment
        .container()
        .start_execution(
            &workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::CleanupScript,
        )
        .await?;

    Ok((
        StatusCode::OK,
        ResponseJson(ApiResponse::success(execution_process)),
    ))
}

#[axum::debug_handler]
pub async fn gh_cli_setup_handler(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<
    (
        StatusCode,
        ResponseJson<ApiResponse<ExecutionProcess, GhCliSetupError>>,
    ),
    ApiError,
> {
    match gh_cli_setup::run_gh_cli_setup(&deployment, &workspace).await {
        Ok(execution_process) => Ok((
            StatusCode::OK,
            ResponseJson(ApiResponse::success(execution_process)),
        )),
        Err(ApiError::Executor(ExecutorError::ExecutableNotFound { program }))
            if program == "brew" =>
        {
            Ok((
                StatusCode::BAD_REQUEST,
                ResponseJson(ApiResponse::error_with_data(GhCliSetupError::BrewMissing)),
            ))
        }
        Err(ApiError::Executor(ExecutorError::SetupHelperNotSupported)) => Ok((
            StatusCode::BAD_REQUEST,
            ResponseJson(ApiResponse::error_with_data(
                GhCliSetupError::SetupHelperNotSupported,
            )),
        )),
        Err(ApiError::Executor(err)) => Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            ResponseJson(ApiResponse::error_with_data(GhCliSetupError::Other {
                message: err.to_string(),
            })),
        )),
        Err(err) => Err(err),
    }
}

pub async fn get_task_attempt_repos(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<RepoWithTargetBranch>>>, ApiError> {
    let pool = &deployment.db().pool;

    let repos =
        WorkspaceRepo::find_repos_with_target_branch_for_workspace(pool, workspace.id).await?;

    Ok(ResponseJson(ApiResponse::success(repos)))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, path::Path};

    use axum::{
        Extension, Json,
        extract::{Query, State},
        http::StatusCode,
        response::Json as ResponseJson,
    };
    use chrono::Utc;
    use db::models::{
        execution_process::{
            CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason,
            ExecutionProcessStatus,
        },
        project::{CreateProject, Project},
        project_repo::{CreateProjectRepo, ProjectRepo},
        repo::Repo,
        session::{CreateSession, Session},
        task::{CreateTask, Task, TaskStatus},
        task_group::{
            TaskGroupEdge, TaskGroupGraph, TaskGroupNode, TaskGroupNodeBaseStrategy,
            TaskGroupNodeKind, TaskGroupNodeLayout,
        },
        workspace::{CreateWorkspace, Workspace},
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    };
    use db_migration::Migrator;
    use deployment::Deployment;
    use executors::{
        actions::{
            ExecutorAction, ExecutorActionType,
            script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
        },
        executors::BaseCodingAgent,
        profile::ExecutorProfileId,
    };
    use local_deployment::container::LocalContainerService;
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;
    use services::services::{
        config::DiffPreviewGuardPreset,
        git::{GitService, GitServiceError},
        workspace_manager::WorkspaceManager,
    };
    use tokio::time::{Duration, sleep};
    use uuid::Uuid;

    use super::{
        AttemptChangesBlockedReason, AttemptChangesQuery, AttemptState, CreateTaskAttemptBody,
        RenameBranchError, RenameBranchRequest, WorkspaceRepoInput, blocked_predecessors,
        cleanup_failed_attempt_start, create_task_attempt, get_task_attempt_changes,
        get_task_attempt_status, normalize_dev_server_working_dir, rename_branch,
        resolve_executor_profile_id, resolve_topology_base_branches, run_git_operation,
        validate_dev_server_script,
    };
    use crate::{
        DeploymentImpl,
        error::ApiError,
        routes::tasks::{CreateAndStartTaskRequest, create_task_and_start},
        test_support::TestEnvGuard,
    };

    fn node(id: &str, status: TaskStatus) -> TaskGroupNode {
        TaskGroupNode {
            id: id.to_string(),
            task_id: Uuid::new_v4(),
            kind: TaskGroupNodeKind::Task,
            phase: 0,
            executor_profile_id: None,
            base_strategy: TaskGroupNodeBaseStrategy::Topology,
            instructions: None,
            requires_approval: None,
            layout: TaskGroupNodeLayout { x: 0.0, y: 0.0 },
            status: Some(status),
        }
    }

    fn node_with_task(id: &str, task_id: Uuid, status: TaskStatus) -> TaskGroupNode {
        TaskGroupNode {
            id: id.to_string(),
            task_id,
            kind: TaskGroupNodeKind::Task,
            phase: 0,
            executor_profile_id: None,
            base_strategy: TaskGroupNodeBaseStrategy::Topology,
            instructions: None,
            requires_approval: None,
            layout: TaskGroupNodeLayout { x: 0.0, y: 0.0 },
            status: Some(status),
        }
    }

    fn node_with_executor(
        id: &str,
        executor_profile_id: Option<ExecutorProfileId>,
    ) -> TaskGroupNode {
        TaskGroupNode {
            id: id.to_string(),
            task_id: Uuid::new_v4(),
            kind: TaskGroupNodeKind::Task,
            phase: 0,
            executor_profile_id,
            base_strategy: TaskGroupNodeBaseStrategy::Topology,
            instructions: None,
            requires_approval: None,
            layout: TaskGroupNodeLayout { x: 0.0, y: 0.0 },
            status: Some(TaskStatus::Todo),
        }
    }

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn create_task(db: &sea_orm::DatabaseConnection, project_id: Uuid, title: &str) -> Uuid {
        let task_id = Uuid::new_v4();
        Task::create(
            db,
            &CreateTask::from_title_description(project_id, title.to_string(), None),
            task_id,
        )
        .await
        .unwrap();
        task_id
    }

    async fn create_workspace_with_repo(
        db: &sea_orm::DatabaseConnection,
        task_id: Uuid,
        repo_id: Uuid,
        workspace_branch: &str,
        target_branch: &str,
    ) -> Workspace {
        let workspace = Workspace::create(
            db,
            &CreateWorkspace {
                branch: workspace_branch.to_string(),
                agent_working_dir: None,
            },
            Uuid::new_v4(),
            task_id,
        )
        .await
        .unwrap();
        WorkspaceRepo::create_many(
            db,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id,
                target_branch: target_branch.to_string(),
            }],
        )
        .await
        .unwrap();
        workspace
    }

    fn list_dir_names(path: &Path) -> HashSet<String> {
        std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn blocked_predecessors_allows_ready_node() {
        let graph = TaskGroupGraph {
            nodes: vec![node("a", TaskStatus::Done), node("b", TaskStatus::Todo)],
            edges: vec![TaskGroupEdge {
                id: "edge-a-b".to_string(),
                from: "a".to_string(),
                to: "b".to_string(),
                data_flow: None,
            }],
        };

        let blocked = blocked_predecessors(&graph, "b").unwrap();
        assert!(blocked.is_empty());
    }

    #[test]
    fn blocked_predecessors_reports_incomplete_nodes() {
        let graph = TaskGroupGraph {
            nodes: vec![
                node("a", TaskStatus::InProgress),
                node("b", TaskStatus::Todo),
            ],
            edges: vec![TaskGroupEdge {
                id: "edge-a-b".to_string(),
                from: "a".to_string(),
                to: "b".to_string(),
                data_flow: None,
            }],
        };

        let blocked = blocked_predecessors(&graph, "b").unwrap();
        assert_eq!(blocked, vec!["a".to_string()]);
    }

    #[test]
    fn blocked_predecessors_requires_existing_node() {
        let graph = TaskGroupGraph {
            nodes: vec![node("a", TaskStatus::Done)],
            edges: Vec::new(),
        };

        let err = blocked_predecessors(&graph, "missing").unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn resolve_executor_profile_id_prefers_node_override() {
        let fallback = ExecutorProfileId::new(BaseCodingAgent::ClaudeCode);
        let override_id =
            ExecutorProfileId::with_variant(BaseCodingAgent::FakeAgent, "TEST".to_string());
        let node = node_with_executor("override", Some(override_id.clone()));

        let resolved = resolve_executor_profile_id(&node, fallback);
        assert_eq!(resolved, override_id);
    }

    #[test]
    fn resolve_executor_profile_id_uses_fallback_when_empty() {
        let fallback = ExecutorProfileId::new(BaseCodingAgent::Codex);
        let node = node_with_executor("fallback", None);

        let resolved = resolve_executor_profile_id(&node, fallback.clone());
        assert_eq!(resolved, fallback);
    }

    #[tokio::test]
    async fn resolve_topology_base_branches_picks_latest_predecessor() {
        let db = setup_db().await;
        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Topology project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&db, Path::new("/tmp/topology-repo"), "Topology")
            .await
            .unwrap();
        let task_a_id = create_task(&db, project_id, "Task A").await;
        let task_b_id = create_task(&db, project_id, "Task B").await;
        let task_c_id = create_task(&db, project_id, "Task C").await;

        create_workspace_with_repo(&db, task_a_id, repo.id, "work-a", "base-a").await;
        sleep(Duration::from_millis(2)).await;
        create_workspace_with_repo(&db, task_b_id, repo.id, "work-b", "base-b").await;

        let graph = TaskGroupGraph {
            nodes: vec![
                node_with_task("a", task_a_id, TaskStatus::Done),
                node_with_task("b", task_b_id, TaskStatus::Done),
                node_with_task("c", task_c_id, TaskStatus::Todo),
            ],
            edges: vec![
                TaskGroupEdge {
                    id: "edge-a-c".to_string(),
                    from: "a".to_string(),
                    to: "c".to_string(),
                    data_flow: None,
                },
                TaskGroupEdge {
                    id: "edge-b-c".to_string(),
                    from: "b".to_string(),
                    to: "c".to_string(),
                    data_flow: None,
                },
            ],
        };

        let branches = resolve_topology_base_branches(&db, &graph, "c")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(branches.get(&repo.id).map(String::as_str), Some("base-b"));
    }

    #[tokio::test]
    async fn resolve_topology_base_branches_skips_incomplete_predecessors() {
        let db = setup_db().await;
        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Topology skipped".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&db, Path::new("/tmp/topology-repo-skip"), "Skip")
            .await
            .unwrap();
        let task_a_id = create_task(&db, project_id, "Task A").await;
        let task_b_id = create_task(&db, project_id, "Task B").await;

        create_workspace_with_repo(&db, task_a_id, repo.id, "work-a", "base-a").await;

        let graph = TaskGroupGraph {
            nodes: vec![
                node_with_task("a", task_a_id, TaskStatus::Todo),
                node_with_task("b", task_b_id, TaskStatus::Todo),
            ],
            edges: vec![TaskGroupEdge {
                id: "edge-a-b".to_string(),
                from: "a".to_string(),
                to: "b".to_string(),
                data_flow: None,
            }],
        };

        let branches = resolve_topology_base_branches(&db, &graph, "b")
            .await
            .unwrap();
        assert!(branches.is_none());
    }

    #[tokio::test]
    async fn start_failure_cleans_up_records_for_attempt_and_create_start() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();
        let repo_path = temp_root.join("repo");
        GitService::new()
            .initialize_repo_with_main_branch(&repo_path)
            .unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Start failure project".to_string(),
                repositories: vec![CreateProjectRepo {
                    display_name: "Repo".to_string(),
                    git_repo_path: repo_path.to_string_lossy().to_string(),
                }],
            },
            project_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&deployment.db().pool, &repo_path, "Repo")
            .await
            .unwrap();
        ProjectRepo::create(&deployment.db().pool, project_id, repo.id)
            .await
            .unwrap();
        let repo_id = repo.id;

        let worktree_base = WorkspaceManager::get_workspace_base_dir();
        std::fs::create_dir_all(&worktree_base).unwrap();
        let baseline_dirs = list_dir_names(&worktree_base);

        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(
                project_id,
                "Attempt failure task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        let attempt_payload = CreateTaskAttemptBody {
            task_id,
            executor_profile_id: ExecutorProfileId::new(BaseCodingAgent::FakeAgent),
            repos: vec![WorkspaceRepoInput {
                repo_id,
                target_branch: "main".to_string(),
            }],
        };

        let attempt_result = create_task_attempt(
            State(deployment.clone()),
            axum::http::HeaderMap::new(),
            Json(attempt_payload),
        )
        .await;
        assert!(attempt_result.is_err());

        let workspaces = Workspace::fetch_all(&deployment.db().pool, Some(task_id))
            .await
            .unwrap();
        assert!(workspaces.is_empty());

        let task_after = Task::find_by_id(&deployment.db().pool, task_id)
            .await
            .unwrap()
            .expect("task should remain");
        assert_eq!(task_after.status, TaskStatus::Todo);

        let after_attempt_dirs = list_dir_names(&worktree_base);
        assert_eq!(baseline_dirs, after_attempt_dirs);

        let create_start_payload = CreateAndStartTaskRequest {
            task: CreateTask::from_title_description(
                project_id,
                "Create start failure task".to_string(),
                None,
            ),
            executor_profile_id: ExecutorProfileId::new(BaseCodingAgent::FakeAgent),
            repos: vec![WorkspaceRepoInput {
                repo_id,
                target_branch: "main".to_string(),
            }],
        };

        let create_start_result =
            create_task_and_start(State(deployment.clone()), Json(create_start_payload)).await;
        assert!(create_start_result.is_err());

        let tasks = Task::find_by_project_id_with_attempt_status(&deployment.db().pool, project_id)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task.id, task_id);

        let after_create_start_dirs = list_dir_names(&worktree_base);
        assert_eq!(baseline_dirs, after_create_start_dirs);

        drop(deployment);
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn create_start_repo_failure_rolls_back_transaction() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Rollback project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let repo_id = Uuid::new_v4();
        let create_start_payload = CreateAndStartTaskRequest {
            task: CreateTask::from_title_description(
                project_id,
                "Create start rollback task".to_string(),
                None,
            ),
            executor_profile_id: ExecutorProfileId::new(BaseCodingAgent::FakeAgent),
            repos: vec![WorkspaceRepoInput {
                repo_id,
                target_branch: "main".to_string(),
            }],
        };

        let result =
            create_task_and_start(State(deployment.clone()), Json(create_start_payload)).await;
        assert!(result.is_err());

        let tasks = Task::find_by_project_id_with_attempt_status(&deployment.db().pool, project_id)
            .await
            .unwrap();
        assert!(tasks.is_empty());

        let workspaces = Workspace::fetch_all(&deployment.db().pool, None)
            .await
            .unwrap();
        assert!(workspaces.is_empty());

        drop(deployment);
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn rename_branch_returns_non_200_with_error_data() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let workspace = Workspace {
            id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            container_ref: None,
            branch: "old-branch".to_string(),
            agent_working_dir: None,
            setup_completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let (status, ResponseJson(response)) = rename_branch(
            Extension(workspace),
            State(deployment),
            Json(RenameBranchRequest {
                new_branch_name: "   ".to_string(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!response.is_success());
        assert!(matches!(
            response.error_data(),
            Some(RenameBranchError::EmptyBranchName)
        ));
    }

    #[tokio::test]
    async fn cleanup_skips_status_restore_when_running_attempt_exists() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Running attempt project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        let task = Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(
                project_id,
                "Running attempt task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        let running_workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "running".to_string(),
                agent_working_dir: None,
            },
            Uuid::new_v4(),
            task_id,
        )
        .await
        .unwrap();

        let session = Session::create(
            &deployment.db().pool,
            &CreateSession { executor: None },
            Uuid::new_v4(),
            running_workspace.id,
        )
        .await
        .unwrap();

        let action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "true".to_string(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: None,
            }),
            None,
        );

        ExecutionProcess::create(
            &deployment.db().pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: action,
                run_reason: ExecutionProcessRunReason::SetupScript,
            },
            Uuid::new_v4(),
            &[],
        )
        .await
        .unwrap();

        let failed_workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "failed".to_string(),
                agent_working_dir: None,
            },
            Uuid::new_v4(),
            task_id,
        )
        .await
        .unwrap();

        Task::update_status(&deployment.db().pool, task_id, TaskStatus::InReview)
            .await
            .unwrap();

        cleanup_failed_attempt_start(&deployment, &task, &failed_workspace, &TaskStatus::Todo)
            .await
            .unwrap();

        let task_after = Task::find_by_id(&deployment.db().pool, task_id)
            .await
            .unwrap()
            .expect("task should remain");
        assert_eq!(task_after.status, TaskStatus::InReview);

        drop(deployment);
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn attempt_status_reports_idle_running_failed_and_ignores_devserver() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Attempt status project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(
                project_id,
                "Attempt status task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        let workspace_id = Uuid::new_v4();
        let workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "attempt-status".to_string(),
                agent_working_dir: None,
            },
            workspace_id,
            task_id,
        )
        .await
        .unwrap();

        let ResponseJson(response) =
            get_task_attempt_status(Extension(workspace.clone()), State(deployment.clone()))
                .await
                .unwrap();
        let status = response.into_data().expect("status should be present");
        assert_eq!(status.state, AttemptState::Idle);
        assert!(status.latest_session_id.is_none());
        assert!(status.latest_execution_process_id.is_none());
        assert!(status.last_activity_at.is_none());

        let session = Session::create(
            &deployment.db().pool,
            &CreateSession { executor: None },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap();

        let action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "true".to_string(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: None,
            }),
            None,
        );

        let process_id = Uuid::new_v4();
        ExecutionProcess::create(
            &deployment.db().pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: action.clone(),
                run_reason: ExecutionProcessRunReason::SetupScript,
            },
            process_id,
            &[],
        )
        .await
        .unwrap();

        let ResponseJson(response) =
            get_task_attempt_status(Extension(workspace.clone()), State(deployment.clone()))
                .await
                .unwrap();
        let status = response.into_data().expect("status should be present");
        assert_eq!(status.state, AttemptState::Running);
        assert_eq!(status.latest_session_id, Some(session.id));
        assert_eq!(status.latest_execution_process_id, Some(process_id));
        assert!(status.failure_summary.is_none());
        assert!(status.last_activity_at.is_some());

        ExecutionProcess::update_completion(
            &deployment.db().pool,
            process_id,
            ExecutionProcessStatus::Failed,
            Some(1),
        )
        .await
        .unwrap();

        let devserver_id = Uuid::new_v4();
        ExecutionProcess::create(
            &deployment.db().pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: action,
                run_reason: ExecutionProcessRunReason::DevServer,
            },
            devserver_id,
            &[],
        )
        .await
        .unwrap();

        let ResponseJson(response) =
            get_task_attempt_status(Extension(workspace), State(deployment))
                .await
                .unwrap();
        let status = response.into_data().expect("status should be present");
        assert_eq!(status.state, AttemptState::Failed);
        assert_eq!(status.latest_execution_process_id, Some(process_id));
        assert!(matches!(
            status.failure_summary.as_deref(),
            Some(summary) if !summary.trim().is_empty()
        ));
        assert!(status.last_activity_at.is_some());

        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn attempt_changes_blocks_when_guard_exceeded_and_unblocks_when_forced() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        {
            let mut config = deployment.config().write().await;
            config.diff_preview_guard = DiffPreviewGuardPreset::Safe;
        }

        let repo_path = temp_root.join("repo");
        GitService::new()
            .initialize_repo_with_main_branch(&repo_path)
            .unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Attempt changes project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&deployment.db().pool, &repo_path, "Repo")
            .await
            .unwrap();
        ProjectRepo::create(&deployment.db().pool, project_id, repo.id)
            .await
            .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(
                project_id,
                "Attempt changes task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        let branch_name = format!("attempt-changes-{}", Uuid::new_v4());
        let workspace_id = Uuid::new_v4();
        let mut workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: branch_name.clone(),
                agent_working_dir: None,
            },
            workspace_id,
            task_id,
        )
        .await
        .unwrap();

        WorkspaceRepo::create_many(
            &deployment.db().pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".to_string(),
            }],
        )
        .await
        .unwrap();

        let workspace_dir_name =
            LocalContainerService::dir_name_from_workspace(&workspace.id, "Attempt changes task");
        let workspace_dir = WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name);
        let _container = WorkspaceManager::create_workspace(
            &workspace_dir,
            &[
                services::services::workspace_manager::RepoWorkspaceInput::new(
                    repo.clone(),
                    "main".to_string(),
                ),
            ],
            &branch_name,
        )
        .await
        .unwrap();

        let worktree_path = workspace_dir.join(&repo.name);
        for i in 0..201 {
            std::fs::write(worktree_path.join(format!("file-{i}.txt")), "hi\n").unwrap();
        }

        workspace.container_ref = Some(workspace_dir.to_string_lossy().to_string());

        let ResponseJson(response) = get_task_attempt_changes(
            Extension(workspace.clone()),
            State(deployment.clone()),
            Query(AttemptChangesQuery { force: false }),
        )
        .await
        .unwrap();
        let changes = response.into_data().expect("changes should be present");
        assert!(changes.blocked);
        assert_eq!(
            changes.blocked_reason,
            Some(AttemptChangesBlockedReason::ThresholdExceeded)
        );
        assert!(changes.files.is_empty());

        let ResponseJson(response) = get_task_attempt_changes(
            Extension(workspace),
            State(deployment),
            Query(AttemptChangesQuery { force: true }),
        )
        .await
        .unwrap();
        let changes = response.into_data().expect("changes should be present");
        assert!(!changes.blocked);
        assert_eq!(changes.blocked_reason, None);
        assert!(
            changes.files.len() >= 201,
            "expected files list to include created files"
        );

        WorkspaceManager::cleanup_workspace(&workspace_dir, &[repo])
            .await
            .unwrap();

        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn cleanup_skips_status_restore_when_latest_attempt_differs() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Completed attempt project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        let task = Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(
                project_id,
                "Completed attempt task".to_string(),
                None,
            ),
            task_id,
        )
        .await
        .unwrap();

        let completed_workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "completed".to_string(),
                agent_working_dir: None,
            },
            Uuid::new_v4(),
            task_id,
        )
        .await
        .unwrap();

        let session = Session::create(
            &deployment.db().pool,
            &CreateSession { executor: None },
            Uuid::new_v4(),
            completed_workspace.id,
        )
        .await
        .unwrap();

        let action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "true".to_string(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: None,
            }),
            None,
        );

        let process_id = Uuid::new_v4();
        ExecutionProcess::create(
            &deployment.db().pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: action,
                run_reason: ExecutionProcessRunReason::SetupScript,
            },
            process_id,
            &[],
        )
        .await
        .unwrap();
        ExecutionProcess::update_completion(
            &deployment.db().pool,
            process_id,
            ExecutionProcessStatus::Completed,
            Some(0),
        )
        .await
        .unwrap();

        let failed_workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "failed".to_string(),
                agent_working_dir: None,
            },
            Uuid::new_v4(),
            task_id,
        )
        .await
        .unwrap();

        Task::update_status(&deployment.db().pool, task_id, TaskStatus::InReview)
            .await
            .unwrap();

        cleanup_failed_attempt_start(&deployment, &task, &failed_workspace, &TaskStatus::Todo)
            .await
            .unwrap();

        let task_after = Task::find_by_id(&deployment.db().pool, task_id)
            .await
            .unwrap()
            .expect("task should remain");
        assert_eq!(task_after.status, TaskStatus::InReview);

        drop(deployment);
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn run_git_operation_does_not_block_async_runtime() {
        let sleep_future = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            1
        });

        let blocking_future = run_git_operation(GitService::new(), |_git| {
            std::thread::sleep(Duration::from_millis(80));
            Ok::<usize, GitServiceError>(2)
        });

        let (sleep_res, blocking_res) = tokio::join!(sleep_future, blocking_future);
        assert_eq!(sleep_res.unwrap(), 1);
        assert_eq!(blocking_res.unwrap(), 2);
    }

    #[test]
    fn normalize_dev_server_working_dir_rejects_escape_path() {
        let root = std::env::temp_dir().join(format!("vk-dev-root-{}", Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("vk-dev-outside-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("repo")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let outside_name = outside
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap();
        let escape_path = format!("../{outside_name}");

        let result = normalize_dev_server_working_dir(&root, Some(&escape_path));
        assert!(matches!(result, Err(ApiError::Forbidden(_))));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn normalize_dev_server_working_dir_accepts_nested_repo_path() {
        let root = std::env::temp_dir().join(format!("vk-dev-root-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("repo-a")).unwrap();

        let result = normalize_dev_server_working_dir(&root, Some("repo-a")).unwrap();
        assert_eq!(result, Some("repo-a".to_string()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_dev_server_script_rejects_shell_operator_script() {
        let result = validate_dev_server_script("npm run dev && rm -rf /");
        assert!(matches!(result, Err(ApiError::BadRequest(_))));
    }
}
