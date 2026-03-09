use std::collections::HashMap;

use app_runtime::Deployment;
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use db::{
    TransactionTrait,
    models::task_group::{CreateTaskGroup, TaskGroup, TaskGroupError, UpdateTaskGroup},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl, error::ApiError, middleware::load_task_group_middleware, routes::task_deletion,
    milestone_dispatch::{milestone_has_active_attempt, next_milestone_dispatch_candidate},
};

#[derive(Debug, Deserialize)]
pub struct TaskGroupQuery {
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum RunNextMilestoneStepStatus {
    Queued,
    QueuedWaitingForActiveAttempt,
    NotEligible,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct RunNextMilestoneStepResponse {
    pub status: RunNextMilestoneStepStatus,
    pub requested_at: Option<DateTime<Utc>>,
    pub candidate_task_id: Option<Uuid>,
    pub message: Option<String>,
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
    let node_instructions = payload
        .graph
        .nodes
        .iter()
        .filter(|node| {
            node.instructions
                .as_ref()
                .is_some_and(|instructions| !instructions.trim().is_empty())
        })
        .count();
    tracing::info!(
        task_group_id = %id,
        project_id = %payload.project_id,
        node_instructions = node_instructions,
        "Creating task group"
    );

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
    if let Some(graph) = payload.graph.as_ref() {
        let node_instructions = graph
            .nodes
            .iter()
            .filter(|node| {
                node.instructions
                    .as_ref()
                    .is_some_and(|instructions| !instructions.trim().is_empty())
            })
            .count();
        tracing::info!(
            task_group_id = %existing.id,
            node_instructions = node_instructions,
            "Updating task group graph"
        );
    } else {
        tracing::info!(task_group_id = %existing.id, "Updating task group metadata");
    }

    let tx = deployment.db().pool.begin().await?;
    let task_group = TaskGroup::update(&tx, existing.id, &payload)
        .await
        .map_err(map_task_group_error)?;
    tx.commit().await?;

    Ok(ResponseJson(ApiResponse::success(task_group)))
}

pub async fn run_next_step(
    Extension(task_group): Extension<TaskGroup>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<RunNextMilestoneStepResponse>>, ApiError> {
    let tasks = db::models::task::Task::find_by_project_id_with_attempt_status(
        &deployment.db().pool,
        task_group.project_id,
    )
    .await?;
    let tasks_by_id: HashMap<Uuid, db::models::task::TaskWithAttemptStatus> =
        tasks.into_iter().map(|task| (task.id, task)).collect();

    if milestone_has_active_attempt(&task_group, &tasks_by_id) {
        let requested_at =
            TaskGroup::request_run_next_step(&deployment.db().pool, task_group.id)
                .await
                .map_err(map_task_group_error)?;
        return Ok(ResponseJson(ApiResponse::success(
            RunNextMilestoneStepResponse {
                status: RunNextMilestoneStepStatus::QueuedWaitingForActiveAttempt,
                requested_at: Some(requested_at),
                candidate_task_id: None,
                message: Some(
                    "Milestone has an active attempt. Next step has been queued and will run after it completes."
                        .to_string(),
                ),
            },
        )));
    }

    let candidate_task_id = next_milestone_dispatch_candidate(&task_group, &tasks_by_id)
        .map(|task| task.id);

    match candidate_task_id {
        Some(candidate_task_id) => {
            let requested_at =
                TaskGroup::request_run_next_step(&deployment.db().pool, task_group.id)
                    .await
                    .map_err(map_task_group_error)?;
            Ok(ResponseJson(ApiResponse::success(
                RunNextMilestoneStepResponse {
                    status: RunNextMilestoneStepStatus::Queued,
                    requested_at: Some(requested_at),
                    candidate_task_id: Some(candidate_task_id),
                    message: None,
                },
            )))
        }
        None => Ok(ResponseJson(ApiResponse::success(RunNextMilestoneStepResponse {
            status: RunNextMilestoneStepStatus::NotEligible,
            requested_at: None,
            candidate_task_id: None,
            message: Some("No eligible milestone node to dispatch right now.".to_string()),
        }))),
    }
}

pub async fn delete_task_group(
    Extension(existing): Extension<TaskGroup>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    task_deletion::delete_task_group_with_cleanup(&deployment, existing, None, false).await?;
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
        .route("/run-next-step", post(run_next_step))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_task_group_middleware::<DeploymentImpl>,
        ));

    let inner = Router::new()
        .route("/", get(get_task_groups).post(create_task_group))
        .nest("/{task_group_id}", task_group_router);

    Router::new().nest("/task-groups", inner)
}
