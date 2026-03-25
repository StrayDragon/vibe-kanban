use std::{path::PathBuf, time::Duration};

use anyhow;
use app_runtime::Deployment;
use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    middleware::from_fn_with_state,
    middleware::Next,
    response::Response,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use chrono::Utc;
use db::models::{
    project::{Project, ProjectFileSearchResponse},
    project_repo::ProjectRepo,
    repo::Repo,
};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use logs_axum::SequencedLogMsgAxumExt;
use repos::file_search_cache::SearchQuery;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const WS_PING_INTERVAL: Duration = Duration::from_secs(30);

fn settings_write_disabled() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        ResponseJson(ApiResponse::<()>::error(
            "Projects settings 已静态化：请编辑 `config.yaml` + reload（POST /api/config/reload）。",
        )),
    )
}

pub async fn get_projects(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<Project>>>, ApiError> {
    let config = deployment.config().read().await;
    let now = Utc::now();
    let projects = config
        .projects
        .iter()
        .filter_map(|project| project_from_config(project, now))
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
    let mut stream = deployment
        .events()
        .stream_projects_raw(after_seq)
        .await?
        .map_ok(|msg| msg.to_ws_message_unchecked());

    let (mut sender, mut receiver) = socket.split();
    let mut ping = tokio::time::interval(WS_PING_INTERVAL);
    ping.tick().await;

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
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if sender.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        continue;
                    }
                    None => break,
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

pub async fn get_project(
    Extension(project): Extension<Project>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
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

#[derive(serde::Deserialize)]
pub struct OpenEditorRequest {
    editor_type: Option<String>,
    git_repo_path: Option<PathBuf>,
}

#[derive(Debug, serde::Serialize, ts_rs::TS)]
pub struct OpenEditorResponse {
    pub url: Option<String>,
}

pub async fn open_project_in_editor(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<Option<OpenEditorRequest>>,
) -> Result<ResponseJson<ApiResponse<OpenEditorResponse>>, ApiError> {
    let path = if let Some(ref req) = payload
        && let Some(ref specified_path) = req.git_repo_path
    {
        specified_path.clone()
    } else {
        let config = deployment.config().read().await;
        let project_config = config
            .projects
            .iter()
            .find(|candidate| candidate.id == Some(project.id))
            .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

        project_config
            .repos
            .first()
            .map(|repo| PathBuf::from(repo.path.clone()))
            .ok_or_else(|| ApiError::BadRequest("Project has no repositories".to_string()))?
    };

    let editor_config = {
        let config = deployment.config().read().await;
        if config.editor.is_integration_disabled() {
            return Err(ApiError::BadRequest(
                "Editor integration is disabled".to_string(),
            ));
        }
        let editor_type_str = payload.as_ref().and_then(|req| req.editor_type.as_deref());
        config.editor.with_override(editor_type_str)
    };

    match editor_config.open_file(&path).await {
        Ok(url) => {
            tracing::info!(
                "Opened editor for project {} at path: {}{}",
                project.id,
                path.to_string_lossy(),
                if url.is_some() { " (remote mode)" } else { "" }
            );

            Ok(ResponseJson(ApiResponse::success(OpenEditorResponse {
                url,
            })))
        }
        Err(e) => {
            tracing::error!("Failed to open editor for project {}: {:?}", project.id, e);
            Err(ApiError::EditorOpen(e))
        }
    }
}

pub async fn search_project_files(
    State(deployment): State<DeploymentImpl>,
    Extension(project): Extension<Project>,
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
    Extension(project): Extension<Project>,
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

        let repo_entity = db::models::repo::Repo::find_or_create(
            &deployment.db().pool,
            &path,
            &display_name,
        )
        .await?;
        repositories.push(repo_entity);
    }
    Ok(ResponseJson(ApiResponse::success(repositories)))
}

pub async fn add_project_repository() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

pub async fn delete_project_repository() -> (StatusCode, ResponseJson<ApiResponse<()>>) {
    settings_write_disabled()
}

pub async fn get_project_repository(
    State(deployment): State<DeploymentImpl>,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<ProjectRepo>>, ApiError> {
    let repo = Repo::find_by_id(&deployment.db().pool, repo_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Repository not found".to_string()))?;

    let repo_path = repo.path.to_string_lossy().to_string();

    let config = deployment.config().read().await;
    let project_config = config
        .projects
        .iter()
        .find(|candidate| candidate.id == Some(project_id))
        .ok_or_else(|| ApiError::NotFound("Project not found".to_string()))?;

    let repo_config = project_config
        .repos
        .iter()
        .find(|candidate| candidate.path == repo_path)
        .ok_or_else(|| {
            ApiError::NotFound("Repository not found in project".to_string())
        })?;

    Ok(ResponseJson(ApiResponse::success(ProjectRepo {
        id: repo_id,
        project_id,
        repo_id,
        setup_script: repo_config.setup_script.clone(),
        cleanup_script: repo_config.cleanup_script.clone(),
        copy_files: repo_config.copy_files.clone(),
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
        .route("/open-editor", post(open_project_in_editor))
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

fn project_from_config(
    project: &config::ProjectConfig,
    now: chrono::DateTime<Utc>,
) -> Option<Project> {
    let id = project.id?;

    let mcp_auto_executor_policy_mode = match project.mcp_auto_executor_policy_mode {
        config::ProjectMcpExecutorPolicyMode::InheritAll => db::types::ProjectMcpExecutorPolicyMode::InheritAll,
        config::ProjectMcpExecutorPolicyMode::AllowList => db::types::ProjectMcpExecutorPolicyMode::AllowList,
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
                config::WorkspaceLifecycleHookFailurePolicy::BlockStart => db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart,
                config::WorkspaceLifecycleHookFailurePolicy::WarnOnly => db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly,
                config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup,
            },
            run_mode: hook.run_mode.as_ref().map(|mode| match mode {
                config::WorkspaceLifecycleHookRunMode::OncePerWorkspace => db::types::WorkspaceLifecycleHookRunMode::OncePerWorkspace,
                config::WorkspaceLifecycleHookRunMode::EveryPrepare => db::types::WorkspaceLifecycleHookRunMode::EveryPrepare,
            }),
        }
    });

    let before_cleanup_hook = project.before_cleanup_hook.as_ref().map(|hook| {
        db::models::project::WorkspaceLifecycleHookConfig {
            command: hook.command.clone(),
            working_dir: hook.working_dir.clone(),
            failure_policy: match hook.failure_policy {
                config::WorkspaceLifecycleHookFailurePolicy::BlockStart => db::types::WorkspaceLifecycleHookFailurePolicy::BlockStart,
                config::WorkspaceLifecycleHookFailurePolicy::WarnOnly => db::types::WorkspaceLifecycleHookFailurePolicy::WarnOnly,
                config::WorkspaceLifecycleHookFailurePolicy::BlockCleanup => db::types::WorkspaceLifecycleHookFailurePolicy::BlockCleanup,
            },
            run_mode: None,
        }
    });

    Some(Project {
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
        remote_project_id: None,
        created_at: now,
        updated_at: now,
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

    let config = deployment.config().read().await;
    let project = config
        .projects
        .iter()
        .find(|candidate| candidate.id == Some(project_id))
        .and_then(|project| project_from_config(project, Utc::now()))
        .ok_or(StatusCode::NOT_FOUND)?;

    request.extensions_mut().insert(project);
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

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
                .contains("config.yaml")
        );
    }
}
