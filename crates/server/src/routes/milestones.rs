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
    models::milestone::{CreateMilestone, Milestone, MilestoneError, UpdateMilestone},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl, error::ApiError, middleware::load_milestone_middleware, routes::task_deletion,
    milestone_dispatch::{milestone_has_active_attempt, next_milestone_dispatch_candidate},
};

#[derive(Debug, Deserialize)]
pub struct MilestoneQuery {
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

fn map_milestone_error(err: MilestoneError) -> ApiError {
    match err {
        MilestoneError::Database(db_err) => ApiError::Database(db_err),
        MilestoneError::MilestoneNotFound => ApiError::BadRequest("Milestone not found".to_string()),
        MilestoneError::ProjectNotFound => ApiError::BadRequest("Project not found".to_string()),
        MilestoneError::Serde(_) => ApiError::BadRequest(err.to_string()),
        MilestoneError::TaskNotFound(_)
        | MilestoneError::TaskProjectMismatch(_)
        | MilestoneError::MilestoneMismatch(_)
        | MilestoneError::TaskKindMismatch(_)
        | MilestoneError::UnsupportedSchemaVersion(_)
        | MilestoneError::InvalidGraph(_) => ApiError::BadRequest(err.to_string()),
    }
}

pub async fn get_milestones(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<MilestoneQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<Milestone>>>, ApiError> {
    let milestones = match query.project_id {
        Some(project_id) => Milestone::find_by_project_id(&deployment.db().pool, project_id)
            .await
            .map_err(map_milestone_error)?,
        None => Milestone::find_all(&deployment.db().pool)
            .await
            .map_err(map_milestone_error)?,
    };

    Ok(ResponseJson(ApiResponse::success(milestones)))
}

pub async fn get_milestone(
    Extension(milestone): Extension<Milestone>,
) -> Result<ResponseJson<ApiResponse<Milestone>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(milestone)))
}

pub async fn create_milestone(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateMilestone>,
) -> Result<ResponseJson<ApiResponse<Milestone>>, ApiError> {
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
        milestone_id = %id,
        project_id = %payload.project_id,
        node_instructions = node_instructions,
        "Creating milestone"
    );

    let tx = deployment.db().pool.begin().await?;
    let milestone = Milestone::create(&tx, &payload, id)
        .await
        .map_err(map_milestone_error)?;
    tx.commit().await?;

    Ok(ResponseJson(ApiResponse::success(milestone)))
}

pub async fn update_milestone(
    Extension(existing): Extension<Milestone>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpdateMilestone>,
) -> Result<ResponseJson<ApiResponse<Milestone>>, ApiError> {
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
            milestone_id = %existing.id,
            node_instructions = node_instructions,
            "Updating milestone graph"
        );
    } else {
        tracing::info!(milestone_id = %existing.id, "Updating milestone metadata");
    }

    let tx = deployment.db().pool.begin().await?;
    let milestone = Milestone::update(&tx, existing.id, &payload)
        .await
        .map_err(map_milestone_error)?;
    tx.commit().await?;

    Ok(ResponseJson(ApiResponse::success(milestone)))
}

pub async fn run_next_step(
    Extension(milestone): Extension<Milestone>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<RunNextMilestoneStepResponse>>, ApiError> {
    let tasks = db::models::task::Task::find_by_project_id_with_attempt_status(
        &deployment.db().pool,
        milestone.project_id,
    )
    .await?;
    let tasks_by_id: HashMap<Uuid, db::models::task::TaskWithAttemptStatus> =
        tasks.into_iter().map(|task| (task.id, task)).collect();

    if milestone_has_active_attempt(&milestone, &tasks_by_id) {
        let requested_at =
            Milestone::request_run_next_step(&deployment.db().pool, milestone.id)
                .await
                .map_err(map_milestone_error)?;
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

    let candidate_task_id = next_milestone_dispatch_candidate(&milestone, &tasks_by_id)
        .map(|task| task.id);

    match candidate_task_id {
        Some(candidate_task_id) => {
            let requested_at =
                Milestone::request_run_next_step(&deployment.db().pool, milestone.id)
                    .await
                    .map_err(map_milestone_error)?;
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

pub async fn delete_milestone(
    Extension(existing): Extension<Milestone>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    task_deletion::delete_milestone_with_cleanup(&deployment, existing, None, false).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let milestone_router = Router::new()
        .route(
            "/",
            get(get_milestone)
                .put(update_milestone)
                .delete(delete_milestone),
        )
        .route("/run-next-step", post(run_next_step))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_milestone_middleware::<DeploymentImpl>,
        ));

    let inner = Router::new()
        .route("/", get(get_milestones).post(create_milestone))
        .nest("/{milestone_id}", milestone_router);

    Router::new().nest("/milestones", inner)
}
