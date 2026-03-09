use std::collections::HashMap;

use db::{
    DbErr, TransactionTrait,
    models::{
        image::TaskImage,
        project::{Project, ProjectError},
        task::{CreateTask, Task, TaskKind, TaskStatus, TaskWithAttemptStatus},
        task_dispatch_state::TaskDispatchState,
        task_group::{
            TaskGroup, TaskGroupError, TaskGroupGraph, TaskGroupNode, TaskGroupNodeBaseStrategy,
        },
        workspace::{CreateWorkspace, Workspace, WorkspaceError},
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    },
};
use executors_protocol::ExecutorProfileId;
use thiserror::Error;
use uuid::Uuid;

use crate::runtime::TaskRuntime;

#[derive(Debug, Clone)]
pub struct CreateAndStartTaskInput {
    pub task: CreateTask,
    pub executor_profile_id: ExecutorProfileId,
    pub repos: Vec<CreateWorkspaceRepo>,
}

#[derive(Debug, Clone)]
pub struct CreateTaskAttemptInput {
    pub task_id: Uuid,
    pub executor_profile_id: ExecutorProfileId,
    pub repos: Vec<CreateWorkspaceRepo>,
    pub prompt_override: Option<String>,
}

#[derive(Debug, Error)]
pub enum TasksError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Runtime(String),
}

fn is_blocking_after_prepare_hook_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("workspace lifecycle hook failed during after_prepare")
}

#[derive(Debug, Clone)]
struct ResolvedAttemptPlan {
    executor_profile_id: ExecutorProfileId,
    repos: Vec<CreateWorkspaceRepo>,
}

pub async fn create_task(db: &db::DbPool, payload: &CreateTask) -> Result<Task, TasksError> {
    let task = Task::create(db, payload, Uuid::new_v4()).await?;

    if let Some(image_ids) = &payload.image_ids {
        TaskImage::associate_many_dedup(db, task.id, image_ids).await?;
    }

    Ok(task)
}

pub async fn create_task_and_start<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    input: &CreateAndStartTaskInput,
) -> Result<TaskWithAttemptStatus, TasksError> {
    if matches!(input.task.task_kind, Some(TaskKind::Group)) {
        return Err(TasksError::BadRequest(
            "Task group entry tasks cannot be started".to_string(),
        ));
    }
    if input.repos.is_empty() {
        return Err(TasksError::BadRequest(
            "At least one repository is required".to_string(),
        ));
    }

    let task_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let git_branch_name = runtime
        .git_branch_from_workspace(attempt_id, &input.task.title)
        .await;

    let tx = db.begin().await?;
    let task = Task::create(&tx, &input.task, task_id).await?;

    if let Some(image_ids) = &input.task.image_ids {
        TaskImage::associate_many_dedup(&tx, task.id, image_ids).await?;
    }

    let project = Project::find_by_id(&tx, task.project_id)
        .await?
        .ok_or(ProjectError::ProjectNotFound)?;

    let agent_working_dir = project
        .default_agent_working_dir
        .as_ref()
        .filter(|dir| !dir.is_empty())
        .cloned();

    let workspace = Workspace::create(
        &tx,
        &CreateWorkspace {
            branch: git_branch_name,
            agent_working_dir,
        },
        attempt_id,
        task.id,
    )
    .await?;

    WorkspaceRepo::create_many(&tx, workspace.id, &input.repos).await?;
    tx.commit().await?;

    if let Err(err) = runtime
        .start_workspace(&workspace, input.executor_profile_id.clone(), None)
        .await
    {
        if is_blocking_after_prepare_hook_error(&err) {
            return Err(TasksError::Conflict(err));
        }
        cleanup_failed_task_start(runtime, db, &task, &workspace).await?;
        return Err(TasksError::Runtime(err));
    }

    let task = Task::find_by_id(db, task.id)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
    let task_id = task.id;

    let task_with_status = TaskWithAttemptStatus {
        task,
        has_in_progress_attempt: true,
        last_attempt_failed: false,
        executor: input.executor_profile_id.executor.to_string(),
        dispatch_state: TaskDispatchState::find_by_task_id(db, task_id).await?,
    };
    Ok(task_with_status)
}

pub async fn create_task_attempt<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    input: &CreateTaskAttemptInput,
) -> Result<Workspace, TasksError> {
    if input.repos.is_empty() {
        return Err(TasksError::BadRequest(
            "At least one repository is required".to_string(),
        ));
    }

    let task = Task::find_by_id(db, input.task_id)
        .await?
        .ok_or(DbErr::RecordNotFound("Task not found".to_string()))?;
    if task.archived_kanban_id.is_some() {
        return Err(TasksError::Conflict(
            "Task is archived. Restore it before starting an attempt.".to_string(),
        ));
    }

    let attempt_plan =
        resolve_attempt_plan(db, &task, input.executor_profile_id.clone(), &input.repos).await?;

    let original_task_status = task.status.clone();
    let project = task
        .parent_project(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

    let agent_working_dir = project
        .default_agent_working_dir
        .as_ref()
        .filter(|dir| !dir.is_empty())
        .cloned();

    let attempt_id = Uuid::new_v4();
    let git_branch_name = runtime
        .git_branch_from_workspace(attempt_id, &task.title)
        .await;

    let tx = db.begin().await?;
    let workspace = Workspace::create(
        &tx,
        &CreateWorkspace {
            branch: git_branch_name,
            agent_working_dir,
        },
        attempt_id,
        input.task_id,
    )
    .await?;

    WorkspaceRepo::create_many(&tx, workspace.id, &attempt_plan.repos).await?;
    tx.commit().await?;

    if let Err(err) = runtime
        .start_workspace(
            &workspace,
            attempt_plan.executor_profile_id,
            input.prompt_override.clone(),
        )
        .await
    {
        if is_blocking_after_prepare_hook_error(&err) {
            return Err(TasksError::Conflict(err));
        }
        cleanup_failed_attempt_start(runtime, db, &task, &workspace, &original_task_status).await?;
        return Err(TasksError::Runtime(err));
    }

    Ok(workspace)
}

fn map_task_group_error(err: TaskGroupError) -> TasksError {
    match err {
        TaskGroupError::Database(db_err) => TasksError::Database(db_err),
        _ => TasksError::BadRequest(err.to_string()),
    }
}

fn map_workspace_error(err: WorkspaceError) -> TasksError {
    match err {
        WorkspaceError::Database(db_err) => TasksError::Database(db_err),
        _ => TasksError::BadRequest(err.to_string()),
    }
}

fn find_task_group_node<'a>(
    graph: &'a TaskGroupGraph,
    node_id: &str,
) -> Result<&'a TaskGroupNode, TasksError> {
    let node_key = node_id.trim();
    if node_key.is_empty() {
        return Err(TasksError::BadRequest(
            "Task group node id cannot be empty".to_string(),
        ));
    }

    graph
        .nodes
        .iter()
        .find(|node| node.id.trim() == node_key)
        .ok_or_else(|| TasksError::BadRequest("Task group node not found in graph".to_string()))
}

fn resolve_executor_profile_id(
    task_group: &TaskGroup,
    task_group_node: &TaskGroupNode,
    fallback: ExecutorProfileId,
) -> ExecutorProfileId {
    if let Some(profile_id) = task_group_node.executor_profile_id.clone() {
        return profile_id;
    }
    if let Some(profile_id) = task_group.default_executor_profile_id.clone() {
        return profile_id;
    }
    fallback
}

async fn resolve_topology_base_branches(
    pool: &db::DbPool,
    graph: &TaskGroupGraph,
    node_id: &str,
) -> Result<Option<HashMap<Uuid, String>>, TasksError> {
    let node_key = node_id.trim();
    if node_key.is_empty() {
        return Err(TasksError::BadRequest(
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
        .map_err(TasksError::Database)?;
    if repos.is_empty() {
        return Ok(None);
    }

    let mut base_branches = HashMap::with_capacity(repos.len());
    for repo in repos {
        base_branches.insert(repo.repo_id, repo.target_branch);
    }

    Ok(Some(base_branches))
}

fn blocked_predecessors(graph: &TaskGroupGraph, node_id: &str) -> Result<Vec<String>, TasksError> {
    let node_key = node_id.trim();
    if node_key.is_empty() {
        return Err(TasksError::BadRequest(
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
        return Err(TasksError::BadRequest(
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

async fn resolve_attempt_plan(
    db: &db::DbPool,
    task: &Task,
    fallback_executor_profile_id: ExecutorProfileId,
    requested_repos: &[CreateWorkspaceRepo],
) -> Result<ResolvedAttemptPlan, TasksError> {
    let mut executor_profile_id = fallback_executor_profile_id;
    let mut baseline_ref: Option<String> = None;
    let mut topology_branches: Option<HashMap<Uuid, String>> = None;

    if let (Some(task_group_id), Some(node_id)) =
        (task.task_group_id, task.task_group_node_id.as_ref())
    {
        let task_group = TaskGroup::find_by_id(db, task_group_id)
            .await
            .map_err(map_task_group_error)?
            .ok_or_else(|| TasksError::BadRequest("Task group not found".to_string()))?;

        let blocked = blocked_predecessors(&task_group.graph, node_id)?;
        if !blocked.is_empty() {
            return Err(TasksError::Conflict(format!(
                "Task is blocked by incomplete predecessors: {}",
                blocked.join(", ")
            )));
        }

        let task_group_node = find_task_group_node(&task_group.graph, node_id)?;
        executor_profile_id =
            resolve_executor_profile_id(&task_group, task_group_node, executor_profile_id);

        match &task_group_node.base_strategy {
            TaskGroupNodeBaseStrategy::Baseline => {
                let trimmed = task_group.baseline_ref.trim();
                if !trimmed.is_empty() {
                    baseline_ref = Some(trimmed.to_string());
                }
            }
            TaskGroupNodeBaseStrategy::Topology => {
                topology_branches =
                    resolve_topology_base_branches(db, &task_group.graph, node_id).await?;
                if topology_branches.is_none() {
                    let trimmed = task_group.baseline_ref.trim();
                    if !trimmed.is_empty() {
                        baseline_ref = Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    let repos = requested_repos
        .iter()
        .map(|repo| CreateWorkspaceRepo {
            repo_id: repo.repo_id,
            target_branch: topology_branches
                .as_ref()
                .and_then(|branches| branches.get(&repo.repo_id))
                .cloned()
                .or_else(|| baseline_ref.clone())
                .unwrap_or_else(|| repo.target_branch.clone()),
        })
        .collect();

    Ok(ResolvedAttemptPlan {
        executor_profile_id,
        repos,
    })
}

async fn cleanup_failed_task_start<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    task: &Task,
    workspace: &Workspace,
) -> Result<(), TasksError> {
    let workspace_for_cleanup = Workspace::find_by_id(db, workspace.id)
        .await?
        .unwrap_or_else(|| workspace.clone());

    if let Err(err) = runtime
        .delete_workspace_container(&workspace_for_cleanup)
        .await
    {
        tracing::error!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            error = %err,
            "Failed to delete workspace worktree after start failure"
        );
    }

    let rows = Task::delete(db, task.id).await?;
    if rows == 0 {
        tracing::warn!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            "Task cleanup skipped because task no longer exists"
        );
    }

    Ok(())
}

async fn cleanup_failed_attempt_start<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    task: &Task,
    workspace: &Workspace,
    original_task_status: &TaskStatus,
) -> Result<(), TasksError> {
    let workspace_for_cleanup = Workspace::find_by_id(db, workspace.id)
        .await?
        .unwrap_or_else(|| workspace.clone());

    if let Err(err) = runtime
        .delete_workspace_container(&workspace_for_cleanup)
        .await
    {
        tracing::error!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            error = %err,
            "Failed to delete workspace worktree after start failure"
        );
    }

    if let Ok(Some(current_task)) = Task::find_by_id(db, task.id).await
        && current_task.status != *original_task_status
        && matches!(
            current_task.status,
            TaskStatus::InProgress | TaskStatus::InReview
        )
    {
        match Task::has_running_attempts(db, task.id).await {
            Ok(false) => {
                let should_restore = match Task::latest_attempt_workspace_id(db, task.id).await {
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
                        Task::update_status(db, task.id, original_task_status.clone()).await
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

    let rows = Workspace::delete(db, workspace.id).await?;
    if rows == 0 {
        tracing::warn!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            "Workspace cleanup skipped because workspace no longer exists"
        );
    }

    Ok(())
}
