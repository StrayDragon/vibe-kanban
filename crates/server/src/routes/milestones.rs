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
use repos::git::{GitCliError, GitService, GitServiceError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::load_milestone_middleware,
    milestone_dispatch::{milestone_has_active_attempt, next_milestone_dispatch_candidate},
    routes::task_deletion,
};

fn resolve_seed_branch(
    git: &GitService,
    repo_path: &std::path::Path,
    preferred: &str,
) -> Option<String> {
    let mut candidates = vec![preferred.trim().to_string()];
    for fallback in ["main", "master"] {
        if !candidates.iter().any(|candidate| candidate == fallback) {
            candidates.push(fallback.to_string());
        }
    }

    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        if git
            .check_branch_exists(repo_path, &candidate)
            .unwrap_or(false)
            || git
                .check_remote_branch_exists(repo_path, &candidate)
                .unwrap_or(false)
        {
            return Some(candidate);
        }
    }

    None
}

async fn ensure_milestone_baseline_branch(
    deployment: &DeploymentImpl,
    milestone: &Milestone,
) -> Result<(), ApiError> {
    let baseline = milestone.baseline_ref.trim();
    if baseline.is_empty() {
        return Err(ApiError::BadRequest(
            "Milestone baseline branch cannot be empty".to_string(),
        ));
    }

    let repos = deployment
        .project()
        .get_repositories(&deployment.db().pool, milestone.project_id)
        .await?;
    if repos.is_empty() {
        return Ok(());
    }

    let preferred_branch = {
        let config = deployment.config().read().await;
        config
            .github
            .default_pr_base
            .clone()
            .unwrap_or_else(|| "main".to_string())
    };

    for repo in repos {
        // If the baseline branch already exists locally, keep it.
        if deployment
            .git()
            .check_branch_exists(&repo.path, baseline)
            .unwrap_or(false)
        {
            continue;
        }

        let Some(seed) = resolve_seed_branch(deployment.git(), &repo.path, &preferred_branch)
        else {
            return Err(ApiError::BadRequest(format!(
                "Base branch unresolved for repo {} ({}); tried preferred, main, master",
                repo.display_name, repo.id
            )));
        };

        deployment
            .git()
            .ensure_local_branch_from_base(&repo.path, baseline, &seed)
            .map_err(ApiError::GitService)?;
    }

    Ok(())
}

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

#[derive(Debug, Clone, Deserialize, TS)]
pub struct PushMilestoneBaselineRequest {
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PushMilestoneBaselineStatus {
    Pushed,
    ForcePushRequired,
    SkippedNoRemote,
    Failed,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct PushMilestoneBaselineRepoResult {
    pub repo_id: Uuid,
    pub repo_display_name: String,
    pub status: PushMilestoneBaselineStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct PushMilestoneBaselineResponse {
    pub branch: String,
    pub results: Vec<PushMilestoneBaselineRepoResult>,
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
        MilestoneError::MilestoneNotFound => {
            ApiError::BadRequest("Milestone not found".to_string())
        }
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
    ensure_milestone_baseline_branch(&deployment, &milestone).await?;
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
    if payload.baseline_ref.is_some() {
        ensure_milestone_baseline_branch(&deployment, &milestone).await?;
    }
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
        let requested_at = Milestone::request_run_next_step(&deployment.db().pool, milestone.id)
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

    let candidate_task_id =
        next_milestone_dispatch_candidate(&milestone, &tasks_by_id).map(|task| task.id);

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
        None => Ok(ResponseJson(ApiResponse::success(
            RunNextMilestoneStepResponse {
                status: RunNextMilestoneStepStatus::NotEligible,
                requested_at: None,
                candidate_task_id: None,
                message: Some("No eligible milestone node to dispatch right now.".to_string()),
            },
        ))),
    }
}

pub async fn push_baseline_branch(
    Extension(milestone): Extension<Milestone>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<PushMilestoneBaselineRequest>,
) -> Result<ResponseJson<ApiResponse<PushMilestoneBaselineResponse>>, ApiError> {
    ensure_milestone_baseline_branch(&deployment, &milestone).await?;

    let repos = deployment
        .project()
        .get_repositories(&deployment.db().pool, milestone.project_id)
        .await?;

    let branch = milestone.baseline_ref.trim().to_string();
    let mut results = Vec::with_capacity(repos.len());

    for repo in repos {
        let mut message: Option<String> = None;
        let status = match deployment
            .git()
            .push_branch_ref(&repo.path, &branch, request.force)
        {
            Ok(()) => PushMilestoneBaselineStatus::Pushed,
            Err(GitServiceError::GitCLI(GitCliError::PushRejected(stderr))) => {
                message = Some(stderr);
                PushMilestoneBaselineStatus::ForcePushRequired
            }
            Err(GitServiceError::InvalidRepository(err))
                if err.contains("remote found") || err.contains("Remote has no URL") =>
            {
                message = Some(err);
                PushMilestoneBaselineStatus::SkippedNoRemote
            }
            Err(err) => {
                message = Some(err.to_string());
                PushMilestoneBaselineStatus::Failed
            }
        };

        results.push(PushMilestoneBaselineRepoResult {
            repo_id: repo.id,
            repo_display_name: repo.display_name,
            status,
            message,
        });
    }

    Ok(ResponseJson(ApiResponse::success(
        PushMilestoneBaselineResponse { branch, results },
    )))
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
        .route("/push-baseline-branch", post(push_baseline_branch))
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
