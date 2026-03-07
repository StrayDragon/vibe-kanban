use app_runtime::Deployment;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::{archived_kanban::ArchivedKanbanWithTaskCount, project::Project};
pub use tasks::archived_kanbans::{
    ArchiveProjectKanbanRequest, ArchiveProjectKanbanResponse, DeleteArchivedKanbanResponse,
    GetArchivedKanbanResponse, RestoreArchivedKanbanRequest, RestoreArchivedKanbanResponse,
};
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, task_runtime::DeploymentTaskRuntime};

pub async fn list_project_archived_kanbans(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ArchivedKanbanWithTaskCount>>>, ApiError> {
    let archives =
        tasks::archived_kanbans::list_project_archived_kanbans(&deployment.db().pool, &project)
            .await?;
    Ok(ResponseJson(ApiResponse::success(archives)))
}

pub async fn archive_project_kanban(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<ArchiveProjectKanbanRequest>,
) -> Result<ResponseJson<ApiResponse<ArchiveProjectKanbanResponse>>, ApiError> {
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    let response = tasks::archived_kanbans::archive_project_kanban(
        &runtime,
        &deployment.db().pool,
        &project,
        &payload,
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

pub async fn get_archived_kanban(
    State(deployment): State<DeploymentImpl>,
    Path(archive_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<GetArchivedKanbanResponse>>, ApiError> {
    let response =
        tasks::archived_kanbans::get_archived_kanban(&deployment.db().pool, archive_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

pub async fn restore_archived_kanban(
    State(deployment): State<DeploymentImpl>,
    Path(archive_id): Path<Uuid>,
    Json(payload): Json<RestoreArchivedKanbanRequest>,
) -> Result<ResponseJson<ApiResponse<RestoreArchivedKanbanResponse>>, ApiError> {
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    let response = tasks::archived_kanbans::restore_archived_kanban(
        &runtime,
        &deployment.db().pool,
        archive_id,
        &payload,
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

pub async fn delete_archived_kanban(
    State(deployment): State<DeploymentImpl>,
    Path(archive_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteArchivedKanbanResponse>>, ApiError> {
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    let response = tasks::archived_kanbans::delete_archived_kanban(
        &runtime,
        &deployment.db().pool,
        archive_id,
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let archive_id_router = Router::new()
        .route("/", get(get_archived_kanban).delete(delete_archived_kanban))
        .route("/restore", post(restore_archived_kanban));

    let _ = deployment;
    Router::new().nest(
        "/archived-kanbans",
        Router::new().nest("/{archive_id}", archive_id_router),
    )
}
