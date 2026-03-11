use std::collections::HashMap;

use app_runtime::Deployment;
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    http::HeaderMap,
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use db::{
    TransactionTrait,
    models::{
        milestone::{CreateMilestone, Milestone, MilestoneError, UpdateMilestone},
        task::{CreateTask, Task},
    },
    types::{TaskCreatedByKind, TaskKind},
};
use repos::git::{GitCliError, GitService, GitServiceError};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    milestone_planning::{
        MILESTONE_PLAN_SCHEMA_VERSION_V1, MilestonePlanApplyResponse, MilestonePlanEdgeKeyV1,
        MilestonePlanMetadataField, MilestonePlanPreviewEdgeDiff, MilestonePlanPreviewMetadataChange,
        MilestonePlanPreviewNodeDiff, MilestonePlanPreviewResponse, MilestonePlanPreviewTaskLink,
        MilestonePlanPreviewTaskToCreate, MilestonePlanV1,
    },
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

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn normalize_optional_text_keep_empty(value: Option<String>) -> Option<String> {
    value.as_ref().map(|v| v.trim().to_string())
}

fn normalize_plan(mut plan: MilestonePlanV1) -> MilestonePlanV1 {
    plan.milestone.objective = normalize_optional_text_keep_empty(plan.milestone.objective);
    plan.milestone.definition_of_done =
        normalize_optional_text_keep_empty(plan.milestone.definition_of_done);
    plan.milestone.baseline_ref = normalize_optional_text_keep_empty(plan.milestone.baseline_ref);

    for node in &mut plan.nodes {
        node.id = node.id.trim().to_string();
        if let Some(create_task) = node.create_task.as_mut() {
            create_task.title = create_task.title.trim().to_string();
            create_task.description = normalize_optional_text(create_task.description.clone());
        }
        node.instructions = normalize_optional_text(node.instructions.clone());
    }

    for edge in &mut plan.edges {
        edge.from = edge.from.trim().to_string();
        edge.to = edge.to.trim().to_string();
        edge.data_flow = normalize_optional_text(edge.data_flow.clone());
    }

    // Make previews + apply deterministic regardless of agent ordering.
    plan.nodes.sort_by(|a, b| a.id.cmp(&b.id));
    plan.edges.sort_by(|a, b| {
        let key_a = (
            a.from.as_str(),
            a.to.as_str(),
            a.data_flow.as_deref().unwrap_or(""),
        );
        let key_b = (
            b.from.as_str(),
            b.to.as_str(),
            b.data_flow.as_deref().unwrap_or(""),
        );
        key_a.cmp(&key_b)
    });

    plan
}

fn validate_plan_dag(
    node_ids: &std::collections::HashSet<String>,
    edges: &[MilestonePlanEdgeKeyV1],
) -> Result<(), ApiError> {
    use std::collections::{HashMap, VecDeque};

    let mut incoming: HashMap<String, usize> =
        node_ids.iter().map(|id| (id.clone(), 0)).collect();
    let mut outgoing: HashMap<String, Vec<String>> =
        node_ids.iter().map(|id| (id.clone(), Vec::new())).collect();

    for edge in edges {
        let from = edge.from.trim();
        let to = edge.to.trim();
        if from.is_empty() || to.is_empty() {
            return Err(ApiError::BadRequest(
                "edge endpoints cannot be empty".to_string(),
            ));
        }
        if !node_ids.contains(from) {
            return Err(ApiError::BadRequest(format!(
                "edge 'from' node does not exist: {from}"
            )));
        }
        if !node_ids.contains(to) {
            return Err(ApiError::BadRequest(format!(
                "edge 'to' node does not exist: {to}"
            )));
        }
        if from == to {
            return Err(ApiError::BadRequest(
                "self edges are not allowed".to_string(),
            ));
        }

        outgoing
            .get_mut(from)
            .ok_or_else(|| ApiError::BadRequest("edge 'from' node missing".to_string()))?
            .push(to.to_string());
        *incoming
            .get_mut(to)
            .ok_or_else(|| ApiError::BadRequest("edge 'to' node missing".to_string()))? += 1;
    }

    let mut queue: VecDeque<String> = incoming
        .iter()
        .filter_map(|(k, v)| if *v == 0 { Some(k.clone()) } else { None })
        .collect();
    let mut visited = 0usize;

    while let Some(node) = queue.pop_front() {
        visited += 1;
        let Some(nexts) = outgoing.get(&node) else { continue };
        for next in nexts {
            let entry = incoming.get_mut(next).ok_or_else(|| {
                ApiError::BadRequest("edge points to unknown node".to_string())
            })?;
            *entry = entry.saturating_sub(1);
            if *entry == 0 {
                queue.push_back(next.clone());
            }
        }
    }

    if visited != node_ids.len() {
        return Err(ApiError::BadRequest(
            "plan contains a cycle; edges must form a DAG".to_string(),
        ));
    }

    Ok(())
}

async fn validate_plan_for_milestone(
    deployment: &DeploymentImpl,
    milestone: &Milestone,
    plan: MilestonePlanV1,
) -> Result<MilestonePlanV1, ApiError> {
    let plan = normalize_plan(plan);

    if plan.schema_version != MILESTONE_PLAN_SCHEMA_VERSION_V1 {
        return Err(ApiError::BadRequest(format!(
            "Unsupported plan schema_version: {}",
            plan.schema_version
        )));
    }

    let mut node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut referenced_task_ids: Vec<Uuid> = Vec::new();
    let mut referenced_task_set: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    for node in &plan.nodes {
        let node_id = node.id.trim();
        if node_id.is_empty() {
            return Err(ApiError::BadRequest(
                "node id cannot be empty".to_string(),
            ));
        }
        if !node_ids.insert(node_id.to_string()) {
            return Err(ApiError::BadRequest(format!(
                "duplicate node id: {node_id}"
            )));
        }

        let has_task = node.task_id.is_some();
        let has_create = node.create_task.is_some();
        if has_task == has_create {
            return Err(ApiError::BadRequest(format!(
                "node {node_id} must specify exactly one of task_id or create_task"
            )));
        }

        if let Some(task_id) = node.task_id {
            if !referenced_task_set.insert(task_id) {
                return Err(ApiError::BadRequest(format!(
                    "duplicate task_id in nodes: {task_id}"
                )));
            }
            referenced_task_ids.push(task_id);
        }

        if let Some(create_task) = &node.create_task
            && create_task.title.trim().is_empty()
        {
            return Err(ApiError::BadRequest(format!(
                "node {node_id} create_task.title cannot be empty"
            )));
        }
    }

    let edge_keys: Vec<MilestonePlanEdgeKeyV1> = plan
        .edges
        .iter()
        .map(|edge| MilestonePlanEdgeKeyV1 {
            from: edge.from.trim().to_string(),
            to: edge.to.trim().to_string(),
            data_flow: normalize_optional_text(edge.data_flow.clone()),
        })
        .collect();

    validate_plan_dag(&node_ids, &edge_keys)?;

    if let Some(baseline_ref) = plan.milestone.baseline_ref.as_ref() {
        let trimmed = baseline_ref.trim();
        if trimmed.is_empty() {
            return Err(ApiError::BadRequest(
                "milestone.baseline_ref cannot be empty".to_string(),
            ));
        }
        if !deployment.git().is_branch_name_valid(trimmed) {
            return Err(ApiError::BadRequest(
                "milestone.baseline_ref is not a valid git branch name".to_string(),
            ));
        }
    }

    if !referenced_task_ids.is_empty() {
        let pool = &deployment.db().pool;
        let project_row_id = db::models::ids::project_id_by_uuid(pool, milestone.project_id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Project not found".to_string()))?;
        let milestone_row_id = db::models::ids::milestone_id_by_uuid(pool, milestone.id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("Milestone not found".to_string()))?;

        let task_models = db::entities::task::Entity::find()
            .filter(db::entities::task::Column::Uuid.is_in(referenced_task_ids.clone()))
            .all(pool)
            .await?;

        let mut task_map: std::collections::HashMap<Uuid, db::entities::task::Model> =
            std::collections::HashMap::new();
        for model in task_models {
            task_map.insert(model.uuid, model);
        }

        for task_id in &referenced_task_ids {
            let Some(task_model) = task_map.get(task_id) else {
                return Err(ApiError::BadRequest(format!("Task not found: {task_id}")));
            };

            if task_model.project_id != project_row_id {
                return Err(ApiError::BadRequest(format!(
                    "Task belongs to another project: {task_id}"
                )));
            }

            if task_model.milestone_id.is_some()
                && task_model.milestone_id != Some(milestone_row_id)
            {
                return Err(ApiError::BadRequest(format!(
                    "Task already linked to another milestone: {task_id}"
                )));
            }

            if task_model.task_kind == TaskKind::Milestone {
                return Err(ApiError::BadRequest(format!(
                    "Task kind 'milestone' cannot be used for milestone nodes: {task_id}"
                )));
            }

            if task_model.archived_kanban_id.is_some() {
                return Err(ApiError::BadRequest(format!(
                    "Task is archived and cannot be linked into a milestone plan: {task_id}"
                )));
            }
        }
    }

    Ok(plan)
}

fn edge_id_for_key(key: &MilestonePlanEdgeKeyV1) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.from.as_bytes());
    hasher.update(b"\n");
    hasher.update(key.to.as_bytes());
    hasher.update(b"\n");
    if let Some(df) = key.data_flow.as_ref() {
        hasher.update(df.as_bytes());
    }
    let digest = hasher.finalize();
    let hex = format!("{digest:x}");
    format!("edge-{}", &hex[..16])
}

fn metadata_field_sort_key(field: &MilestonePlanMetadataField) -> u8 {
    match field {
        MilestonePlanMetadataField::Objective => 0,
        MilestonePlanMetadataField::DefinitionOfDone => 1,
        MilestonePlanMetadataField::DefaultExecutorProfile => 2,
        MilestonePlanMetadataField::AutomationMode => 3,
        MilestonePlanMetadataField::BaselineRef => 4,
    }
}

fn executor_profile_label(profile: &executors_protocol::ExecutorProfileId) -> String {
    let variant = profile.variant.clone().unwrap_or_default();
    format!("{}::{}", profile.executor, variant)
}

fn normalized_optional_field(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn edge_tuple(edge: &MilestonePlanEdgeKeyV1) -> (String, String, String) {
    (
        edge.from.clone(),
        edge.to.clone(),
        edge.data_flow.clone().unwrap_or_default(),
    )
}

fn compute_preview_response(
    milestone: &Milestone,
    plan: &MilestonePlanV1,
) -> MilestonePlanPreviewResponse {
    let mut metadata_changes: Vec<MilestonePlanPreviewMetadataChange> = Vec::new();

    if plan.milestone.objective.is_some() {
        let planned = normalized_optional_field(&plan.milestone.objective);
        if planned != milestone.objective {
            metadata_changes.push(MilestonePlanPreviewMetadataChange {
                field: MilestonePlanMetadataField::Objective,
                from: milestone.objective.clone(),
                to: planned,
            });
        }
    }

    if plan.milestone.definition_of_done.is_some() {
        let planned = normalized_optional_field(&plan.milestone.definition_of_done);
        if planned != milestone.definition_of_done {
            metadata_changes.push(MilestonePlanPreviewMetadataChange {
                field: MilestonePlanMetadataField::DefinitionOfDone,
                from: milestone.definition_of_done.clone(),
                to: planned,
            });
        }
    }

    if let Some(default_profile_patch) = plan.milestone.default_executor_profile_id.clone()
        && default_profile_patch != milestone.default_executor_profile_id
    {
        metadata_changes.push(MilestonePlanPreviewMetadataChange {
            field: MilestonePlanMetadataField::DefaultExecutorProfile,
            from: milestone
                .default_executor_profile_id
                .as_ref()
                .map(executor_profile_label),
            to: default_profile_patch.as_ref().map(executor_profile_label),
        });
    }

    if let Some(mode) = plan.milestone.automation_mode.clone()
        && mode != milestone.automation_mode
    {
        metadata_changes.push(MilestonePlanPreviewMetadataChange {
            field: MilestonePlanMetadataField::AutomationMode,
            from: Some(milestone.automation_mode.to_string()),
            to: Some(mode.to_string()),
        });
    }

    if let Some(baseline_ref) = plan.milestone.baseline_ref.clone() {
        let trimmed = baseline_ref.trim().to_string();
        if trimmed != milestone.baseline_ref {
            metadata_changes.push(MilestonePlanPreviewMetadataChange {
                field: MilestonePlanMetadataField::BaselineRef,
                from: Some(milestone.baseline_ref.clone()),
                to: Some(trimmed),
            });
        }
    }

    metadata_changes.sort_by_key(|change| metadata_field_sort_key(&change.field));

    let mut tasks_to_create: Vec<MilestonePlanPreviewTaskToCreate> = Vec::new();
    let mut task_links: Vec<MilestonePlanPreviewTaskLink> = Vec::new();

    for node in &plan.nodes {
        if let Some(task_id) = node.task_id {
            task_links.push(MilestonePlanPreviewTaskLink {
                node_id: node.id.clone(),
                task_id,
            });
        } else if let Some(create_task) = &node.create_task {
            tasks_to_create.push(MilestonePlanPreviewTaskToCreate {
                node_id: node.id.clone(),
                title: create_task.title.clone(),
                description: create_task.description.clone(),
            });
        }
    }

    tasks_to_create.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    task_links.sort_by(|a, b| a.node_id.cmp(&b.node_id));

    let mut existing_nodes: Vec<String> = milestone
        .graph
        .nodes
        .iter()
        .map(|n| n.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    existing_nodes.sort();

    let mut planned_nodes: Vec<String> = plan
        .nodes
        .iter()
        .map(|n| n.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    planned_nodes.sort();

    let existing_set: std::collections::HashSet<String> =
        existing_nodes.iter().cloned().collect();
    let planned_set: std::collections::HashSet<String> =
        planned_nodes.iter().cloned().collect();

    let mut added_nodes: Vec<String> = planned_set.difference(&existing_set).cloned().collect();
    let mut removed_nodes: Vec<String> =
        existing_set.difference(&planned_set).cloned().collect();
    added_nodes.sort();
    removed_nodes.sort();

    let node_diff = MilestonePlanPreviewNodeDiff {
        existing: existing_nodes,
        planned: planned_nodes,
        added: added_nodes,
        removed: removed_nodes,
    };

    let mut existing_edges: Vec<MilestonePlanEdgeKeyV1> = milestone
        .graph
        .edges
        .iter()
        .map(|e| MilestonePlanEdgeKeyV1 {
            from: e.from.trim().to_string(),
            to: e.to.trim().to_string(),
            data_flow: normalize_optional_text(e.data_flow.clone()),
        })
        .collect();
    existing_edges.sort_by_key(edge_tuple);

    let mut planned_edges: Vec<MilestonePlanEdgeKeyV1> = plan
        .edges
        .iter()
        .map(|e| MilestonePlanEdgeKeyV1 {
            from: e.from.trim().to_string(),
            to: e.to.trim().to_string(),
            data_flow: normalize_optional_text(e.data_flow.clone()),
        })
        .collect();
    planned_edges.sort_by_key(edge_tuple);

    let existing_edge_set: std::collections::HashSet<(String, String, String)> =
        existing_edges.iter().map(edge_tuple).collect();
    let planned_edge_set: std::collections::HashSet<(String, String, String)> =
        planned_edges.iter().map(edge_tuple).collect();

    let mut added_edges: Vec<MilestonePlanEdgeKeyV1> = planned_edge_set
        .difference(&existing_edge_set)
        .map(|(from, to, df)| MilestonePlanEdgeKeyV1 {
            from: from.clone(),
            to: to.clone(),
            data_flow: if df.is_empty() { None } else { Some(df.clone()) },
        })
        .collect();
    added_edges.sort_by_key(edge_tuple);

    let mut removed_edges: Vec<MilestonePlanEdgeKeyV1> = existing_edge_set
        .difference(&planned_edge_set)
        .map(|(from, to, df)| MilestonePlanEdgeKeyV1 {
            from: from.clone(),
            to: to.clone(),
            data_flow: if df.is_empty() { None } else { Some(df.clone()) },
        })
        .collect();
    removed_edges.sort_by_key(edge_tuple);

    let edge_diff = MilestonePlanPreviewEdgeDiff {
        existing: existing_edges,
        planned: planned_edges,
        added: added_edges,
        removed: removed_edges,
    };

    MilestonePlanPreviewResponse {
        metadata_changes,
        tasks_to_create,
        task_links,
        node_diff,
        edge_diff,
    }
}

pub async fn preview_milestone_plan(
    Extension(milestone): Extension<Milestone>,
    State(deployment): State<DeploymentImpl>,
    Json(plan): Json<MilestonePlanV1>,
) -> Result<ResponseJson<ApiResponse<MilestonePlanPreviewResponse>>, ApiError> {
    let plan = validate_plan_for_milestone(&deployment, &milestone, plan).await?;
    let preview = compute_preview_response(&milestone, &plan);
    Ok(ResponseJson(ApiResponse::success(preview)))
}

pub async fn apply_milestone_plan(
    Extension(milestone): Extension<Milestone>,
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    Json(plan): Json<MilestonePlanV1>,
) -> Result<ResponseJson<ApiResponse<MilestonePlanApplyResponse>>, ApiError> {
    #[derive(Serialize)]
    struct ApplyIdempotencyPayload<'a> {
        milestone_id: Uuid,
        plan: &'a MilestonePlanV1,
    }

    let key = crate::routes::idempotency::idempotency_key(&headers);
    let hash = crate::routes::idempotency::request_hash(&ApplyIdempotencyPayload {
        milestone_id: milestone.id,
        plan: &plan,
    })?;

    let deployment_for_apply = deployment.clone();
    let milestone_for_apply = milestone.clone();
    crate::routes::idempotency::idempotent_success(
        &deployment.db().pool,
        "milestone_plan_apply",
        key.clone(),
        hash,
        move || {
            let deployment = deployment_for_apply.clone();
            let milestone = milestone_for_apply.clone();
            let key = key.clone();
            let plan = plan.clone();
            async move {
                let normalized_plan =
                    validate_plan_for_milestone(&deployment, &milestone, plan).await?;

                // Pre-resolve task creations deterministically.
                let mut node_create_specs: Vec<(String, CreateTask)> = Vec::new();
                for node in &normalized_plan.nodes {
                    if let Some(create_task) = &node.create_task {
                        node_create_specs.push((
                            node.id.clone(),
                            CreateTask::from_title_description(
                                milestone.project_id,
                                create_task.title.clone(),
                                create_task.description.clone(),
                            ),
                        ));
                    }
                }
                node_create_specs.sort_by(|a, b| a.0.cmp(&b.0));

                let tx = deployment.db().pool.begin().await?;

                let mut created_tasks: Vec<Task> = Vec::new();
                let mut created_task_ids_by_node: std::collections::HashMap<String, Uuid> =
                    std::collections::HashMap::new();

                for (node_id, mut create_payload) in node_create_specs {
                    create_payload.task_kind = Some(TaskKind::Default);
                    create_payload.created_by_kind = Some(TaskCreatedByKind::MilestonePlanner);
                    let new_task_id = Uuid::new_v4();
                    let task = Task::create(&tx, &create_payload, new_task_id).await?;
                    created_task_ids_by_node.insert(node_id, new_task_id);
                    created_tasks.push(task);
                }

                // Stable fallback layout: group by phase, then node id.
                let mut nodes_for_layout = normalized_plan.nodes.clone();
                nodes_for_layout.sort_by(|a, b| (a.phase, a.id.as_str()).cmp(&(b.phase, b.id.as_str())));

                let mut fallback_layout_by_node: std::collections::HashMap<
                    String,
                    db::models::milestone::MilestoneNodeLayout,
                > = std::collections::HashMap::new();
                let mut phase_counts: std::collections::HashMap<i32, usize> =
                    std::collections::HashMap::new();

                for node in &nodes_for_layout {
                    let idx = phase_counts.entry(node.phase).or_insert(0);
                    let i = *idx;
                    *idx += 1;
                    let col = (i % 4) as f64;
                    let row = (i / 4) as f64;
                    let phase_y = node.phase as f64 * 220.0;
                    fallback_layout_by_node.insert(
                        node.id.clone(),
                        db::models::milestone::MilestoneNodeLayout {
                            x: col * 260.0,
                            y: phase_y + row * 180.0,
                        },
                    );
                }

                let mut resolved_nodes: Vec<db::models::milestone::MilestoneNode> = Vec::new();
                for node in &normalized_plan.nodes {
                    let task_id = match (node.task_id, node.create_task.as_ref()) {
                        (Some(existing), None) => existing,
                        (None, Some(_)) => *created_task_ids_by_node
                            .get(&node.id)
                            .ok_or_else(|| {
                                ApiError::Internal("Missing created task id".to_string())
                            })?,
                        _ => {
                            return Err(ApiError::BadRequest(format!(
                                "node {} must specify exactly one of task_id or create_task",
                                node.id
                            )));
                        }
                    };

                    let layout = node.layout.clone().unwrap_or_else(|| {
                        fallback_layout_by_node
                            .get(&node.id)
                            .cloned()
                            .unwrap_or(db::models::milestone::MilestoneNodeLayout { x: 0.0, y: 0.0 })
                    });

                    resolved_nodes.push(db::models::milestone::MilestoneNode {
                        id: node.id.clone(),
                        task_id,
                        kind: node.kind.clone(),
                        phase: node.phase,
                        executor_profile_id: node.executor_profile_id.clone(),
                        base_strategy: node.base_strategy.clone(),
                        instructions: normalize_optional_text(node.instructions.clone()),
                        requires_approval: node.requires_approval,
                        layout,
                        status: None,
                    });
                }

                let mut resolved_edges: Vec<db::models::milestone::MilestoneEdge> = Vec::new();
                for edge in &normalized_plan.edges {
                    let key = MilestonePlanEdgeKeyV1 {
                        from: edge.from.trim().to_string(),
                        to: edge.to.trim().to_string(),
                        data_flow: normalize_optional_text(edge.data_flow.clone()),
                    };
                    resolved_edges.push(db::models::milestone::MilestoneEdge {
                        id: edge_id_for_key(&key),
                        from: key.from,
                        to: key.to,
                        data_flow: key.data_flow,
                    });
                }

                let graph = db::models::milestone::MilestoneGraph {
                    nodes: resolved_nodes,
                    edges: resolved_edges,
                };

                let update = UpdateMilestone {
                    title: None,
                    description: None,
                    objective: normalized_plan.milestone.objective.clone(),
                    definition_of_done: normalized_plan.milestone.definition_of_done.clone(),
                    default_executor_profile_id: normalized_plan
                        .milestone
                        .default_executor_profile_id
                        .clone(),
                    automation_mode: normalized_plan.milestone.automation_mode.clone(),
                    status: None,
                    baseline_ref: normalized_plan.milestone.baseline_ref.clone(),
                    schema_version: None,
                    graph: Some(graph),
                };

                let updated = Milestone::update(&tx, milestone.id, &update)
                    .await
                    .map_err(map_milestone_error)?;
                ensure_milestone_baseline_branch(&deployment, &updated).await?;

                let plan_json = serde_json::to_string(&normalized_plan).map_err(|e| {
                    ApiError::Internal(format!("Failed to serialize applied plan payload: {e}"))
                })?;

                let application = db::models::milestone_plan_application::create(
                    &tx,
                    milestone.id,
                    normalized_plan.schema_version,
                    plan_json,
                    TaskCreatedByKind::HumanUi,
                    key.clone(),
                    Uuid::new_v4(),
                )
                .await?;

                tx.commit().await?;

                let milestone_fresh = Milestone::find_by_id(&deployment.db().pool, milestone.id)
                    .await
                    .map_err(map_milestone_error)?
                    .ok_or_else(|| ApiError::BadRequest("Milestone not found".to_string()))?;

                Ok(MilestonePlanApplyResponse {
                    milestone: milestone_fresh,
                    created_tasks,
                    applied_at: application.applied_at,
                })
            }
        },
    )
    .await
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
        .route("/plan/preview", post(preview_milestone_plan))
        .route("/plan/apply", post(apply_milestone_plan))
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

#[cfg(test)]
mod tests {
    use app_runtime::Deployment;
    use axum::{Extension, Json, extract::State, http::HeaderValue};
    use db::models::{
        milestone::{CreateMilestone, MilestoneGraph},
        project::{CreateProject, Project},
        task::Task,
    };
    use uuid::Uuid;

    use super::{apply_milestone_plan, preview_milestone_plan};
    use crate::{DeploymentImpl, milestone_planning::MilestonePlanV1, test_support::TestEnvGuard};

    fn idempotency_headers(key: &'static str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Idempotency-Key", HeaderValue::from_static(key));
        headers
    }

    async fn setup_deployment() -> (TestEnvGuard, DeploymentImpl) {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);
        let deployment = DeploymentImpl::new().await.unwrap();
        (env_guard, deployment)
    }

    async fn create_project_and_milestone(
        deployment: &DeploymentImpl,
        project_id: Uuid,
        milestone_id: Uuid,
    ) -> db::models::milestone::Milestone {
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "planning".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        db::models::milestone::Milestone::create(
            &deployment.db().pool,
            &CreateMilestone {
                project_id,
                title: "milestone".to_string(),
                description: None,
                objective: None,
                definition_of_done: None,
                default_executor_profile_id: None,
                automation_mode: Some(db::types::MilestoneAutomationMode::Manual),
                status: None,
                baseline_ref: Some("main".to_string()),
                schema_version: 1,
                graph: MilestoneGraph {
                    nodes: Vec::new(),
                    edges: Vec::new(),
                },
            },
            milestone_id,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn preview_rejects_unsupported_schema_version() {
        let (_guard, deployment) = setup_deployment().await;
        let project_id = Uuid::new_v4();
        let milestone_id = Uuid::new_v4();
        let milestone = create_project_and_milestone(&deployment, project_id, milestone_id).await;

        let plan = MilestonePlanV1 {
            schema_version: 999,
            milestone: Default::default(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        let err = preview_milestone_plan(
            Extension(milestone),
            State(deployment),
            Json(plan),
        )
        .await
        .expect_err("expected bad request");

        assert!(matches!(err, crate::error::ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn apply_is_idempotent_and_persists_provenance() {
        let (_guard, deployment) = setup_deployment().await;
        let project_id = Uuid::new_v4();
        let milestone_id = Uuid::new_v4();
        let milestone = create_project_and_milestone(&deployment, project_id, milestone_id).await;

        let plan = MilestonePlanV1 {
            schema_version: 1,
            milestone: crate::milestone_planning::MilestonePlanMilestonePatchV1 {
                objective: Some("Ship v1".to_string()),
                definition_of_done: Some("All tests pass".to_string()),
                default_executor_profile_id: None,
                automation_mode: None,
                baseline_ref: None,
            },
            nodes: vec![
                crate::milestone_planning::MilestonePlanNodeV1 {
                    id: "node-a".to_string(),
                    kind: db::models::milestone::MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: db::models::milestone::MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: None,
                    task_id: None,
                    create_task: Some(crate::milestone_planning::MilestonePlanCreateTaskV1 {
                        title: "Implement".to_string(),
                        description: Some("do it".to_string()),
                    }),
                },
                crate::milestone_planning::MilestonePlanNodeV1 {
                    id: "node-b".to_string(),
                    kind: db::models::milestone::MilestoneNodeKind::Checkpoint,
                    phase: 1,
                    executor_profile_id: None,
                    base_strategy: db::models::milestone::MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: Some(true),
                    layout: None,
                    task_id: None,
                    create_task: Some(crate::milestone_planning::MilestonePlanCreateTaskV1 {
                        title: "Review".to_string(),
                        description: None,
                    }),
                },
            ],
            edges: vec![crate::milestone_planning::MilestonePlanEdgeV1 {
                from: "node-a".to_string(),
                to: "node-b".to_string(),
                data_flow: None,
            }],
        };

        let response1 = apply_milestone_plan(
            Extension(milestone.clone()),
            State(deployment.clone()),
            idempotency_headers("req-1"),
            Json(plan.clone()),
        )
        .await
        .unwrap();
        let data1 = response1.0.into_data().expect("apply response");

        let response2 = apply_milestone_plan(
            Extension(milestone),
            State(deployment.clone()),
            idempotency_headers("req-1"),
            Json(plan),
        )
        .await
        .unwrap();
        let data2 = response2.0.into_data().expect("apply response");

        assert_eq!(
            data1
                .created_tasks
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            data2
                .created_tasks
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>()
        );

        // Entry task + 2 node tasks.
        let tasks =
            Task::find_by_project_id_with_attempt_status(&deployment.db().pool, project_id)
                .await
                .unwrap();
        assert_eq!(tasks.len(), 3);

        assert!(
            data1.milestone.last_plan_application.is_some(),
            "expected provenance to be surfaced in milestone reads"
        );
        assert!(
            data1.created_tasks
                .iter()
                .all(|t| t.created_by_kind == db::types::TaskCreatedByKind::MilestonePlanner)
        );
    }

    #[tokio::test]
    async fn invalid_apply_does_not_create_tasks() {
        let (_guard, deployment) = setup_deployment().await;
        let project_id = Uuid::new_v4();
        let milestone_id = Uuid::new_v4();
        let milestone = create_project_and_milestone(&deployment, project_id, milestone_id).await;

        let plan = MilestonePlanV1 {
            schema_version: 1,
            milestone: Default::default(),
            nodes: vec![
                crate::milestone_planning::MilestonePlanNodeV1 {
                    id: "a".to_string(),
                    kind: db::models::milestone::MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: db::models::milestone::MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: None,
                    task_id: None,
                    create_task: Some(crate::milestone_planning::MilestonePlanCreateTaskV1 {
                        title: "A".to_string(),
                        description: None,
                    }),
                },
                crate::milestone_planning::MilestonePlanNodeV1 {
                    id: "b".to_string(),
                    kind: db::models::milestone::MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: db::models::milestone::MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: None,
                    task_id: None,
                    create_task: Some(crate::milestone_planning::MilestonePlanCreateTaskV1 {
                        title: "B".to_string(),
                        description: None,
                    }),
                },
            ],
            edges: vec![
                crate::milestone_planning::MilestonePlanEdgeV1 {
                    from: "a".to_string(),
                    to: "b".to_string(),
                    data_flow: None,
                },
                crate::milestone_planning::MilestonePlanEdgeV1 {
                    from: "b".to_string(),
                    to: "a".to_string(),
                    data_flow: None,
                },
            ],
        };

        let err = apply_milestone_plan(
            Extension(milestone),
            State(deployment.clone()),
            idempotency_headers("req-2"),
            Json(plan),
        )
        .await
        .expect_err("expected bad request");

        assert!(matches!(err, crate::error::ApiError::BadRequest(_)));

        // Only the entry task should exist.
        let tasks =
            Task::find_by_project_id_with_attempt_status(&deployment.db().pool, project_id)
                .await
                .unwrap();
        assert_eq!(tasks.len(), 1);
    }
}
