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
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
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

use crate::{DeploymentImpl, error::ApiError, middleware::load_project_middleware};

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
    let projects = Project::find_all(&deployment.db().pool).await?;
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
        let repositories = deployment
            .project()
            .get_repositories(&deployment.db().pool, project.id)
            .await?;

        repositories
            .first()
            .map(|r| r.path.clone())
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

    let repositories = deployment
        .project()
        .get_repositories(&deployment.db().pool, project.id)
        .await?;

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
    let repositories = deployment
        .project()
        .get_repositories(&deployment.db().pool, project.id)
        .await?;
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
    match ProjectRepo::find_by_project_and_repo(&deployment.db().pool, project_id, repo_id).await {
        Ok(Some(project_repo)) => Ok(ResponseJson(ApiResponse::success(project_repo))),
        Ok(None) => Err(ApiError::NotFound(
            "Repository not found in project".to_string(),
        )),
        Err(e) => Err(e.into()),
    }
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
            load_project_middleware::<DeploymentImpl>,
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
