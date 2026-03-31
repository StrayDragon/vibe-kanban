use std::{path::PathBuf, time::Duration};

use anyhow;
use app_runtime::Deployment;
use axum::{
    Extension, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Json as ResponseJson, Response},
    routing::get,
};
use chrono::Utc;
use db::models::{project::ProjectFileSearchResponse, repo::Repo};
use futures_util::{SinkExt, StreamExt};
use json_patch::{PatchOperation, ReplaceOperation};
use logs_axum::SequencedLogMsgAxumExt;
use logs_protocol::LogMsg;
use logs_store::SequencedLogMsg;
use repos::file_search_cache::SearchQuery;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const WS_PING_INTERVAL: Duration = Duration::from_secs(30);

fn settings_write_disabled() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        ResponseJson(ApiResponse::<()>::error(
            "Projects settings 已静态化：请编辑 `projects.yaml`（或 `projects.d/*.yaml`）+ reload（POST /api/config/reload）。",
        )),
    )
}

pub async fn get_projects(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ProjectPublic>>>, ApiError> {
    let config = deployment.public_config().read().await.clone();
    let projects = config
        .projects
        .iter()
        .filter_map(project_public_from_config)
        .collect();
    Ok(ResponseJson(ApiResponse::success(projects)))
}

pub async fn stream_projects_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ProjectsStreamQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_projects_ws(socket, deployment, query.after_seq).await {
            tracing::warn!("projects WS closed: {}", e);
        }
    })
}

#[derive(Debug, serde::Deserialize)]
pub struct ProjectsStreamQuery {
    pub after_seq: Option<u64>,
}

async fn handle_projects_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
    after_seq: Option<u64>,
) -> anyhow::Result<()> {
    let shutdown = deployment.shutdown_token();
    let (mut sender, mut receiver) = socket.split();
    let mut ping = tokio::time::interval(WS_PING_INTERVAL);
    ping.tick().await;
    let mut reload_poll = tokio::time::interval(Duration::from_secs(2));
    reload_poll.tick().await;

    let mut last_loaded_at = deployment.config_status().read().await.loaded_at;
    let mut last_seq = after_seq.unwrap_or(0);
    send_projects_snapshot(&mut sender, &deployment, &mut last_seq).await?;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            _ = ping.tick() => {
                if sender.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
            _ = reload_poll.tick() => {
                let loaded_at = deployment.config_status().read().await.loaded_at;
                if loaded_at != last_loaded_at {
                    last_loaded_at = loaded_at;
                    if send_projects_snapshot(&mut sender, &deployment, &mut last_seq).await.is_err() {
                        break;
                    }
                }
            }
            msg = receiver.next() => {
                if msg.is_none() {
                    break;
                }
            }
        }
    }

    let _ = sender.close().await;
    Ok(())
}

fn next_ws_seq(last_seq: &mut u64) -> u64 {
    fn now_millis() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};

        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64
    }

    let now = now_millis();
    let next = last_seq.saturating_add(1).max(now);
    *last_seq = next;
    next
}

async fn send_projects_snapshot(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    deployment: &DeploymentImpl,
    last_seq: &mut u64,
) -> anyhow::Result<()> {
    let config = deployment.public_config().read().await.clone();
    let projects = config
        .projects
        .iter()
        .filter_map(project_public_from_config)
        .collect::<Vec<_>>();

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

    let patch = json_patch::Patch(vec![PatchOperation::Replace(ReplaceOperation {
        path: "/projects"
            .try_into()
            .expect("projects snapshot path should be valid"),
        value: serde_json::Value::Object(projects_map),
    })]);

    let seq = next_ws_seq(last_seq);
    let msg = SequencedLogMsg {
        seq,
        msg: LogMsg::JsonPatch(patch).into(),
    };
    sender.send(msg.to_ws_message_unchecked()).await?;
    Ok(())
}

pub async fn get_project(
    Extension(project): Extension<ProjectPublic>,
) -> Result<ResponseJson<ApiResponse<ProjectPublic>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(project)))
}

pub async fn create_project() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

pub async fn update_project() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

pub async fn delete_project() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

pub async fn search_project_files(
    State(deployment): State<DeploymentImpl>,
    Extension(project): Extension<ProjectPublic>,
    Query(search_query): Query<SearchQuery>,
) -> Result<ResponseJson<ApiResponse<ProjectFileSearchResponse>>, ApiError> {
    if search_query.q.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Query parameter 'q' is required and cannot be empty".to_string(),
        ));
    }

    let config = deployment.config().read().await;
    let project_config = config
        .projects
        .iter()
        .find(|candidate| candidate.id == Some(project.id))
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let now = Utc::now();
    let repositories: Vec<Repo> = project_config
        .repos
        .iter()
        .map(|repo| Repo {
            id: Uuid::new_v4(),
            path: PathBuf::from(repo.path.clone()),
            name: repo
                .display_name
                .clone()
                .or_else(|| {
                    PathBuf::from(repo.path.clone())
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "repo".to_string()),
            display_name: repo
                .display_name
                .clone()
                .or_else(|| {
                    PathBuf::from(repo.path.clone())
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "repo".to_string()),
            created_at: now,
            updated_at: now,
        })
        .collect();

    let results = deployment
        .project()
        .search_files(
            deployment.file_search_cache().as_ref(),
            &repositories,
            &search_query,
        )
        .await?;

    Ok(ResponseJson(ApiResponse::success(results)))
}

pub async fn get_project_repositories(
    Extension(project): Extension<ProjectPublic>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<Repo>>>, ApiError> {
    let config = deployment.config().read().await;
    let project_config = config
        .projects
        .iter()
        .find(|candidate| candidate.id == Some(project.id))
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let mut repositories = Vec::with_capacity(project_config.repos.len());
    for repo in &project_config.repos {
        let path = PathBuf::from(repo.path.clone());
        let display_name = repo
            .display_name
            .clone()
            .or_else(|| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "repo".to_string());

        let repo_entity =
            db::models::repo::Repo::find_or_create(&deployment.db().pool, &path, &display_name)
                .await?;
        repositories.push(repo_entity);
    }
    Ok(ResponseJson(ApiResponse::success(repositories)))
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct AddProjectRepositoryRequest {
    #[serde(alias = "git_repo_path")]
    pub path: String,
    pub display_name: Option<String>,
    pub reload: Option<bool>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct AddProjectRepositoryResponse {
    pub project_id: Uuid,
    pub path: String,
    pub display_name: String,
    pub used_git_root: bool,
    pub was_added: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ProjectsUiFile {
    #[serde(default)]
    project_repo_overrides: Vec<ProjectRepoOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRepoOverride {
    project_id: Uuid,
    #[serde(default)]
    repos: Vec<ProjectRepoOverrideRepo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRepoOverrideRepo {
    path: String,
    display_name: Option<String>,
}

fn normalize_repo_path_key(path: &str) -> String {
    let trimmed = path.trim();
    let trimmed = trimmed.trim_end_matches(['/', '\\']);
    let normalized =
        utils_core::path::normalize_macos_private_alias(std::path::Path::new(trimmed));
    normalized.to_string_lossy().to_string()
}

fn detect_git_toplevel(dir: &std::path::Path) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let toplevel = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if toplevel.is_empty() {
        return None;
    }
    Some(PathBuf::from(toplevel))
}

fn atomic_write_text(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let tmp_path = parent.join(format!(".{file_name}.tmp-{}", Uuid::new_v4()));
    std::fs::write(&tmp_path, contents)?;

    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Windows may fail to rename over an existing file; fall back to remove+rename.
            let _ = std::fs::remove_file(path);
            std::fs::rename(&tmp_path, path).map_err(|_| {
                let _ = std::fs::remove_file(&tmp_path);
                err
            })
        }
    }
}

fn load_projects_ui_file(path: &std::path::Path) -> Result<ProjectsUiFile, ApiError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_yaml::from_str::<ProjectsUiFile>(&raw).map_err(|err| {
            ApiError::BadRequest(format!(
                "Invalid {}: {err}",
                path.to_string_lossy()
            ))
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ProjectsUiFile::default()),
        Err(err) => Err(ApiError::BadRequest(format!(
            "Failed to read {}: {err}",
            path.to_string_lossy()
        ))),
    }
}

pub async fn add_project_repository(
    Extension(project): Extension<ProjectPublic>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<AddProjectRepositoryRequest>,
) -> Result<ResponseJson<ApiResponse<AddProjectRepositoryResponse>>, ApiError> {
    let input_path = payload.path.trim();
    if input_path.is_empty() {
        return Err(ApiError::BadRequest("path must not be empty".to_string()));
    }

    let normalized = std::path::absolute(utils_core::path::expand_tilde(input_path))
        .map_err(|err| ApiError::BadRequest(format!("Invalid path: {err}")))?;
    if !normalized.exists() {
        return Err(ApiError::BadRequest(format!(
            "Path does not exist: {}",
            normalized.display()
        )));
    }
    if !normalized.is_dir() {
        return Err(ApiError::BadRequest(format!(
            "Path is not a directory: {}",
            normalized.display()
        )));
    }

    let (repo_path, used_git_root) = detect_git_toplevel(&normalized)
        .filter(|path| path.is_absolute() && path.is_dir())
        .map(|path| (path, true))
        .unwrap_or((normalized, false));

    let display_name = payload
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or_else(|| {
            repo_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "repo".to_string());

    let repo_path_str = repo_path.to_string_lossy().to_string();
    let repo_path_key = normalize_repo_path_key(&repo_path_str);

    let already_in_config = {
        let config = deployment.config().read().await;
        config
            .projects
            .iter()
            .find(|candidate| candidate.id == Some(project.id))
            .is_some_and(|project_config| {
                project_config
                    .repos
                    .iter()
                    .any(|repo| normalize_repo_path_key(&repo.path) == repo_path_key)
            })
    };
    if already_in_config {
        return Ok(ResponseJson(ApiResponse::success(AddProjectRepositoryResponse {
            project_id: project.id,
            path: repo_path_str,
            display_name,
            used_git_root,
            was_added: false,
        })));
    }

    let ui_path = utils_core::vk_projects_ui_yaml_path();
    let mut ui_file = load_projects_ui_file(&ui_path)?;

    let override_entry = ui_file
        .project_repo_overrides
        .iter_mut()
        .find(|entry| entry.project_id == project.id);
    let override_entry = match override_entry {
        Some(entry) => entry,
        None => {
            ui_file.project_repo_overrides.push(ProjectRepoOverride {
                project_id: project.id,
                repos: Vec::new(),
            });
            ui_file
                .project_repo_overrides
                .last_mut()
                .expect("just pushed")
        }
    };

    let mut was_added = false;
    let already_exists = override_entry.repos.iter().any(|repo| {
        normalize_repo_path_key(&repo.path) == repo_path_key
    });
    if !already_exists {
        override_entry.repos.push(ProjectRepoOverrideRepo {
            path: repo_path_str.clone(),
            display_name: Some(display_name.clone()),
        });
        was_added = true;
    }

    ui_file
        .project_repo_overrides
        .sort_by_key(|entry| entry.project_id);
    for entry in &mut ui_file.project_repo_overrides {
        entry.repos.sort_by(|a, b| a.path.cmp(&b.path));
    }

    let serialized = serde_yaml::to_string(&ui_file).map_err(|err| {
        ApiError::BadRequest(format!("Failed to serialize projects.ui.yaml content: {err}"))
    })?;
    atomic_write_text(&ui_path, &serialized).map_err(|err| {
        ApiError::BadRequest(format!(
            "Failed to write {}: {err}",
            ui_path.to_string_lossy()
        ))
    })?;

    if payload.reload.unwrap_or(true) {
        deployment.reload_user_config().await.map_err(|err| {
            ApiError::BadRequest(format!("Config reload failed after writing projects.ui.yaml: {err}"))
        })?;
    }

    Ok(ResponseJson(ApiResponse::success(AddProjectRepositoryResponse {
        project_id: project.id,
        path: repo_path_str,
        display_name,
        used_git_root,
        was_added,
    })))
}

pub async fn delete_project_repository() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectRepoPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub repo_id: Uuid,
    pub has_setup_script: bool,
    pub has_cleanup_script: bool,
    pub has_copy_files: bool,
    pub parallel_setup_script: bool,
}

pub async fn get_project_repository(
    State(deployment): State<DeploymentImpl>,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<ProjectRepoPublic>>, ApiError> {
    let repo = Repo::find_by_id(&deployment.db().pool, repo_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Repository not found".to_string()))?;

    let repo_path = repo.path.to_string_lossy().to_string();

    let config = deployment.public_config().read().await;
    let project_config = config
        .projects
        .iter()
        .find(|candidate| candidate.id == Some(project_id))
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let repo_config = project_config
        .repos
        .iter()
        .find(|candidate| candidate.path == repo_path)
        .ok_or_else(|| ApiError::NotFound("Repository not found in project".to_string()))?;

    let has_setup_script = repo_config
        .setup_script
        .as_deref()
        .is_some_and(|script| !script.trim().is_empty());
    let has_cleanup_script = repo_config
        .cleanup_script
        .as_deref()
        .is_some_and(|script| !script.trim().is_empty());
    let has_copy_files = repo_config
        .copy_files
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());

    Ok(ResponseJson(ApiResponse::success(ProjectRepoPublic {
        id: repo_id,
        project_id,
        repo_id,
        has_setup_script,
        has_cleanup_script,
        has_copy_files,
        parallel_setup_script: repo_config.parallel_setup_script,
    })))
}

pub async fn update_project_repository() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let project_id_router = Router::new()
        .route(
            "/",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/search", get(search_project_files))
        .route(
            "/archived-kanbans",
            get(crate::routes::archived_kanbans::list_project_archived_kanbans)
                .post(crate::routes::archived_kanbans::archive_project_kanban),
        )
        .route(
            "/repositories",
            get(get_project_repositories).post(add_project_repository),
        )
        .layer(from_fn_with_state(
            deployment.clone(),
            load_project_from_config_middleware,
        ));

    let projects_router = Router::new()
        .route("/", get(get_projects).post(create_project))
        .route(
            "/{project_id}/repositories/{repo_id}",
            get(get_project_repository)
                .put(update_project_repository)
                .delete(delete_project_repository),
        )
        .route("/stream/ws", get(stream_projects_ws))
        .nest("/{id}", project_id_router);

    Router::new().nest("/projects", projects_router)
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectPublic {
    pub id: Uuid,
    pub name: String,
    pub dev_script: Option<String>,
    pub dev_script_working_dir: Option<String>,
    pub default_agent_working_dir: Option<String>,
    pub git_no_verify_override: Option<bool>,
    pub scheduler_max_concurrent: i32,
    pub scheduler_max_retries: i32,
    pub default_continuation_turns: i32,
    pub mcp_auto_executor_policy_mode: db::types::ProjectMcpExecutorPolicyMode,
    pub mcp_auto_executor_policy_allow_list: Vec<db::types::ProjectExecutorProfileAllowListEntry>,
    pub after_prepare_hook: Option<db::models::project::WorkspaceLifecycleHookConfig>,
    pub before_cleanup_hook: Option<db::models::project::WorkspaceLifecycleHookConfig>,
    pub remote_project_id: Option<Uuid>,
}

pub(crate) fn project_public_from_config(project: &config::ProjectConfig) -> Option<ProjectPublic> {
    let id = project.id?;

    let mcp_auto_executor_policy_mode = match project.mcp_auto_executor_policy_mode {
        config::ProjectMcpExecutorPolicyMode::InheritAll => {
            db::types::ProjectMcpExecutorPolicyMode::InheritAll
        }
        config::ProjectMcpExecutorPolicyMode::AllowList => {
            db::types::ProjectMcpExecutorPolicyMode::AllowList
        }
    };

    let mcp_auto_executor_policy_allow_list = project
        .mcp_auto_executor_policy_allow_list
        .iter()
        .map(|entry| db::types::ProjectExecutorProfileAllowListEntry {
            executor: entry.executor.to_string(),
            variant: entry.variant.clone(),
        })
        .collect();

    let after_prepare_hook = project.after_prepare_hook.as_ref().map(|hook| {
        db::models::project::WorkspaceLifecycleHookConfig {
            command: hook.command.clone(),
            working_dir: hook.working_dir.clone(),
            failure_policy: match hook.failure_policy {
                config::WorkspaceLifecycleHookFailurePolicy::BlockStart => {
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart
                }
                config::WorkspaceLifecycleHookFailurePolicy::WarnOnly => {
                    db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly
                }
                config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup
                }
            },
            run_mode: hook.run_mode.as_ref().map(|mode| match mode {
                config::WorkspaceLifecycleHookRunMode::OncePerWorkspace => {
                    db::types::WorkspaceLifecycleHookRunMode::OncePerWorkspace
                }
                config::WorkspaceLifecycleHookRunMode::EveryPrepare => {
                    db::types::WorkspaceLifecycleHookRunMode::EveryPrepare
                }
            }),
        }
    });

    let before_cleanup_hook = project.before_cleanup_hook.as_ref().map(|hook| {
        db::models::project::WorkspaceLifecycleHookConfig {
            command: hook.command.clone(),
            working_dir: hook.working_dir.clone(),
            failure_policy: match hook.failure_policy {
                config::WorkspaceLifecycleHookFailurePolicy::BlockStart => {
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart
                }
                config::WorkspaceLifecycleHookFailurePolicy::WarnOnly => {
                    db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly
                }
                config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => {
                    db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup
                }
            },
            run_mode: None,
        }
    });

    Some(ProjectPublic {
        id,
        name: project.name.clone(),
        dev_script: project.dev_script.clone(),
        dev_script_working_dir: project.dev_script_working_dir.clone(),
        default_agent_working_dir: project.default_agent_working_dir.clone(),
        git_no_verify_override: project.git_no_verify_override,
        scheduler_max_concurrent: project.scheduler_max_concurrent,
        scheduler_max_retries: project.scheduler_max_retries,
        default_continuation_turns: project.default_continuation_turns,
        mcp_auto_executor_policy_mode,
        mcp_auto_executor_policy_allow_list,
        after_prepare_hook,
        before_cleanup_hook,
        remote_project_id: project.remote_project_id,
    })
}

async fn load_project_from_config_middleware(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if request.method() == axum::http::Method::PUT || request.method() == axum::http::Method::DELETE
    {
        return Ok(next.run(request).await);
    }

    let config = deployment.public_config().read().await.clone();
    let project = config
        .projects
        .iter()
        .find(|candidate| candidate.id == Some(project_id))
        .and_then(project_public_from_config)
        .ok_or(StatusCode::NOT_FOUND)?;

    // Ensure the DB has a minimal `projects` row for this configured project.
    // Many tables have FKs to `projects`, so "read-only config" still needs a
    // small cache row to exist before first write.
    db::models::project::Project::find_or_create_minimal(
        &deployment.db().pool,
        project.id,
        &project.name,
    )
    .await
    .map_err(|err| {
        tracing::error!(project_id = %project.id, error = %err, "Failed to ensure project row");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    request.extensions_mut().insert(project);
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use db::models::{
        project::{CreateProject, Project},
        repo::Repo,
    };
    use test_support::{TempRoot, TestDb, TestEnvGuard};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn create_project_is_disabled() {
        let (status, ResponseJson(response)) = create_project().await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert!(!response.is_success());
        assert!(
            response
                .message()
                .unwrap_or_default()
                .contains("projects.yaml")
        );
    }

    #[tokio::test]
    async fn add_project_repository_writes_projects_ui_yaml_and_reload() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let vk_config_dir = env_guard.vk_config_dir().to_path_buf();
        let repo_a = temp_root.join("repo-a");
        let repo_b = temp_root.join("repo-b");
        fs::create_dir_all(&repo_a).unwrap();
        fs::create_dir_all(&repo_b).unwrap();

        let project_id = Uuid::new_v4();
        fs::write(
            vk_config_dir.join("projects.yaml"),
            format!(
                r#"projects:
  - id: "{project_id}"
    name: "Test"
    repos:
      - path: "{}"
"#,
                repo_a.to_string_lossy()
            ),
        )
        .unwrap();

        let deployment = DeploymentImpl::new().await.unwrap();
        let app = crate::http::router(deployment);

        let payload = serde_json::json!({
            "path": repo_b.to_string_lossy(),
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/repositories"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.pointer("/data/was_added"),
            Some(&serde_json::Value::Bool(true))
        );

        let ui_path = vk_config_dir.join("projects.ui.yaml");
        assert!(ui_path.is_file());
        let ui_raw = fs::read_to_string(&ui_path).unwrap();
        assert!(ui_raw.contains(repo_b.to_string_lossy().as_ref()));

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/projects/{project_id}/repositories"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.pointer("/data").and_then(|v| v.as_array()).map(|v| v.len()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn add_project_repository_rejects_missing_path() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let vk_config_dir = env_guard.vk_config_dir().to_path_buf();
        let repo_a = temp_root.join("repo-a");
        fs::create_dir_all(&repo_a).unwrap();

        let project_id = Uuid::new_v4();
        fs::write(
            vk_config_dir.join("projects.yaml"),
            format!(
                r#"projects:
  - id: "{project_id}"
    name: "Test"
    repos:
      - path: "{}"
"#,
                repo_a.to_string_lossy()
            ),
        )
        .unwrap();

        let deployment = DeploymentImpl::new().await.unwrap();
        let app = crate::http::router(deployment);

        let payload = serde_json::json!({
            "path": temp_root.join("does-not-exist").to_string_lossy(),
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/repositories"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn add_project_repository_is_idempotent() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let vk_config_dir = env_guard.vk_config_dir().to_path_buf();
        let repo_a = temp_root.join("repo-a");
        let repo_b = temp_root.join("repo-b");
        fs::create_dir_all(&repo_a).unwrap();
        fs::create_dir_all(&repo_b).unwrap();

        let project_id = Uuid::new_v4();
        fs::write(
            vk_config_dir.join("projects.yaml"),
            format!(
                r#"projects:
  - id: "{project_id}"
    name: "Test"
    repos:
      - path: "{}"
"#,
                repo_a.to_string_lossy()
            ),
        )
        .unwrap();

        let deployment = DeploymentImpl::new().await.unwrap();
        let app = crate::http::router(deployment);

        let payload = serde_json::json!({
            "path": repo_b.to_string_lossy(),
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/repositories"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/repositories"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.pointer("/data/was_added"),
            Some(&serde_json::Value::Bool(false))
        );

        let ui_path = vk_config_dir.join("projects.ui.yaml");
        let ui_raw = fs::read_to_string(&ui_path).unwrap();
        let ui_file: ProjectsUiFile = serde_yaml::from_str(&ui_raw).unwrap();
        let entry = ui_file
            .project_repo_overrides
            .iter()
            .find(|e| e.project_id == project_id)
            .unwrap();
        assert_eq!(entry.repos.len(), 1);
    }

    #[tokio::test]
    async fn add_project_repository_returns_error_on_write_failure() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let vk_config_dir = env_guard.vk_config_dir().to_path_buf();
        let repo_a = temp_root.join("repo-a");
        let repo_b = temp_root.join("repo-b");
        fs::create_dir_all(&repo_a).unwrap();
        fs::create_dir_all(&repo_b).unwrap();

        let project_id = Uuid::new_v4();
        fs::write(
            vk_config_dir.join("projects.yaml"),
            format!(
                r#"projects:
  - id: "{project_id}"
    name: "Test"
    repos:
      - path: "{}"
"#,
                repo_a.to_string_lossy()
            ),
        )
        .unwrap();

        let deployment = DeploymentImpl::new().await.unwrap();
        let app = crate::http::router(deployment);

        // Make the overlay path non-writable by turning it into a directory (after config load).
        fs::create_dir_all(vk_config_dir.join("projects.ui.yaml")).unwrap();

        let payload = serde_json::json!({
            "path": repo_b.to_string_lossy(),
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/repositories"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn project_routes_do_not_leak_expanded_secrets() {
        let secret = "sekrit-value-123";

        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        // Set up a projects.yaml that references a secret placeholder in fields that may be
        // returned by API routes (dev_script) and in repo setup_script (should not be exposed).
        let vk_config_dir = temp_root.join("vk-config");
        let repo_path = temp_root.join("repo");
        fs::create_dir_all(&repo_path).unwrap();

        let secret_env_path = vk_config_dir.join("secret.env");
        fs::write(&secret_env_path, format!("MY_SECRET={secret}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut perms = fs::metadata(&secret_env_path).unwrap().permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&secret_env_path, perms).unwrap();
        }

        let project_id = Uuid::new_v4();
        let projects_yaml = format!(
            r#"projects:
  - id: "{project_id}"
    name: "SecretTest"
    dev_script: "echo {{{{secret.MY_SECRET}}}}"
    repos:
      - path: "{}"
        display_name: "Repo"
        setup_script: "echo {{{{secret.MY_SECRET}}}}"
"#,
            repo_path.to_string_lossy()
        );
        fs::write(vk_config_dir.join("projects.yaml"), projects_yaml).unwrap();

        let deployment = DeploymentImpl::new().await.unwrap();

        let repo = Repo::find_or_create(&deployment.db().pool, &repo_path, "Repo")
            .await
            .unwrap();

        let app = crate::http::router(deployment);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Public config should preserve placeholders (no template expansion) and must not contain
        // the expanded secret value.
        let dev_script = json
            .pointer("/data/0/dev_script")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert!(
            dev_script.contains("{{secret.MY_SECRET}}"),
            "dev_script did not preserve placeholders: {dev_script:?}\nbody: {body_str}"
        );
        assert!(
            !body_str.contains(secret),
            "response leaked expanded secret value\nbody: {body_str}"
        );

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/projects/{project_id}/repositories/{}",
                        repo.id
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        assert!(!body_str.contains(secret));

        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.pointer("/data/has_setup_script"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[tokio::test]
    async fn projects_endpoint_uses_yaml_as_source_of_truth() {
        let temp_root = TempRoot::new("vk-test-");
        let db = TestDb::sqlite_file(&temp_root);
        let _env_guard = TestEnvGuard::new(temp_root.path(), db.url().to_string());

        let vk_config_dir = temp_root.join("vk-config");
        let repo_path = temp_root.join("repo");
        fs::create_dir_all(&repo_path).unwrap();

        let project_id = Uuid::new_v4();
        fs::write(
            vk_config_dir.join("projects.yaml"),
            format!(
                r#"projects:
  - id: "{project_id}"
    name: "YAML Name"
    repos:
      - path: "{}"
"#,
                repo_path.to_string_lossy()
            ),
        )
        .unwrap();

        let deployment = DeploymentImpl::new().await.unwrap();

        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "DB Name".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let app = crate::http::router(deployment);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let project_id_str = project_id.to_string();
        assert_eq!(
            json.pointer("/data/0/id").and_then(|v| v.as_str()),
            Some(project_id_str.as_str())
        );
        assert_eq!(
            json.pointer("/data/0/name").and_then(|v| v.as_str()),
            Some("YAML Name")
        );
    }
}
