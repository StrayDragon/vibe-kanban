use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::get,
};
use db::{
    TransactionTrait,
    models::task_group::{CreateTaskGroup, TaskGroup, TaskGroupError, UpdateTaskGroup},
};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl, error::ApiError, middleware::load_task_group_middleware, routes::task_deletion,
};

#[derive(Debug, Deserialize)]
pub struct TaskGroupQuery {
    pub project_id: Option<Uuid>,
}

fn map_task_group_error(err: TaskGroupError) -> ApiError {
    match err {
        TaskGroupError::Database(db_err) => ApiError::Database(db_err),
        TaskGroupError::TaskGroupNotFound => {
            ApiError::BadRequest("Task group not found".to_string())
        }
        TaskGroupError::ProjectNotFound => ApiError::BadRequest("Project not found".to_string()),
        TaskGroupError::Serde(_) => ApiError::BadRequest(err.to_string()),
        TaskGroupError::TaskNotFound(_)
        | TaskGroupError::TaskProjectMismatch(_)
        | TaskGroupError::TaskGroupMismatch(_)
        | TaskGroupError::TaskKindMismatch(_)
        | TaskGroupError::UnsupportedSchemaVersion(_)
        | TaskGroupError::InvalidGraph(_) => ApiError::BadRequest(err.to_string()),
    }
}

pub async fn get_task_groups(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskGroupQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskGroup>>>, ApiError> {
    let groups = match query.project_id {
        Some(project_id) => TaskGroup::find_by_project_id(&deployment.db().pool, project_id)
            .await
            .map_err(map_task_group_error)?,
        None => TaskGroup::find_all(&deployment.db().pool)
            .await
            .map_err(map_task_group_error)?,
    };

    Ok(ResponseJson(ApiResponse::success(groups)))
}

pub async fn get_task_group(
    Extension(task_group): Extension<TaskGroup>,
) -> Result<ResponseJson<ApiResponse<TaskGroup>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(task_group)))
}

pub async fn create_task_group(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateTaskGroup>,
) -> Result<ResponseJson<ApiResponse<TaskGroup>>, ApiError> {
    let id = Uuid::new_v4();
    let tx = deployment.db().pool.begin().await?;
    let task_group = TaskGroup::create(&tx, &payload, id)
        .await
        .map_err(map_task_group_error)?;
    tx.commit().await?;

    Ok(ResponseJson(ApiResponse::success(task_group)))
}

pub async fn update_task_group(
    Extension(existing): Extension<TaskGroup>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpdateTaskGroup>,
) -> Result<ResponseJson<ApiResponse<TaskGroup>>, ApiError> {
    let tx = deployment.db().pool.begin().await?;
    let task_group = TaskGroup::update(&tx, existing.id, &payload)
        .await
        .map_err(map_task_group_error)?;
    tx.commit().await?;

    Ok(ResponseJson(ApiResponse::success(task_group)))
}

pub async fn delete_task_group(
    Extension(existing): Extension<TaskGroup>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    task_deletion::delete_task_group_with_cleanup(&deployment, existing, None).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let task_group_router = Router::new()
        .route(
            "/",
            get(get_task_group)
                .put(update_task_group)
                .delete(delete_task_group),
        )
        .layer(from_fn_with_state(
            deployment.clone(),
            load_task_group_middleware,
        ));

    let inner = Router::new()
        .route("/", get(get_task_groups).post(create_task_group))
        .nest("/{task_group_id}", task_group_router);

    Router::new().nest("/task-groups", inner)
}
