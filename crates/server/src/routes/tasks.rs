use anyhow;
use axum::{
    Extension, Json, Router,
    extract::{
        Query, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{delete, get, post, put},
};
use db::{DbErr, TransactionTrait};
use db::models::{
    image::TaskImage,
    project::{Project, ProjectError},
    task::{CreateTask, Task, TaskKind, TaskWithAttemptStatus, UpdateTask},
    workspace::{CreateWorkspace, Workspace},
    workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
};
use deployment::Deployment;
use executors::profile::ExecutorProfileId;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl, error::ApiError, middleware::load_task_middleware,
    routes::{task_attempts::WorkspaceRepoInput, task_deletion},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskQuery {
    pub project_id: Option<Uuid>,
}

pub async fn get_tasks(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskWithAttemptStatus>>>, ApiError> {
    let tasks = match query.project_id {
        Some(project_id) => {
            Task::find_by_project_id_with_attempt_status(&deployment.db().pool, project_id).await?
        }
        None => Task::find_all_with_attempt_status(&deployment.db().pool).await?,
    };

    Ok(ResponseJson(ApiResponse::success(tasks)))
}

pub async fn stream_tasks_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_tasks_ws(socket, deployment, query.project_id).await {
            tracing::warn!("tasks WS closed: {}", e);
        }
    })
}

async fn handle_tasks_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
    project_id: Option<Uuid>,
) -> anyhow::Result<()> {
    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = deployment
        .events()
        .stream_tasks_raw(project_id)
        .await?
        .map_ok(|msg| msg.to_ws_message_unchecked());

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Drain (and ignore) any client->server messages so pings/pongs work
    tokio::spawn(async move { while let Some(Ok(_)) = receiver.next().await {} });

    // Forward server messages
    while let Some(item) = stream.next().await {
        match item {
            Ok(msg) => {
                if sender.send(msg).await.is_err() {
                    break; // client disconnected
                }
            }
            Err(e) => {
                tracing::error!("stream error: {}", e);
                continue;
            }
        }
    }
    let _ = sender.close().await;
    Ok(())
}

pub async fn get_task(
    Extension(task): Extension<Task>,
    State(_deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Task>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(task)))
}

pub async fn create_task(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateTask>,
) -> Result<ResponseJson<ApiResponse<Task>>, ApiError> {
    let id = Uuid::new_v4();

    tracing::debug!(
        "Creating task '{}' in project {}",
        payload.title,
        payload.project_id
    );

    let task = Task::create(&deployment.db().pool, &payload, id).await?;

    if let Some(image_ids) = &payload.image_ids {
        TaskImage::associate_many_dedup(&deployment.db().pool, task.id, image_ids).await?;
    }

    Ok(ResponseJson(ApiResponse::success(task)))
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateAndStartTaskRequest {
    pub task: CreateTask,
    pub executor_profile_id: ExecutorProfileId,
    pub repos: Vec<WorkspaceRepoInput>,
}

pub async fn create_task_and_start(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateAndStartTaskRequest>,
) -> Result<ResponseJson<ApiResponse<TaskWithAttemptStatus>>, ApiError> {
    if matches!(payload.task.task_kind, Some(TaskKind::Group)) {
        return Err(ApiError::BadRequest(
            "Task group entry tasks cannot be started".to_string(),
        ));
    }
    if payload.repos.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one repository is required".to_string(),
        ));
    }

    let pool = &deployment.db().pool;

    let task_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let git_branch_name = deployment
        .container()
        .git_branch_from_workspace(&attempt_id, &payload.task.title)
        .await;

    let tx = pool.begin().await?;
    let task = Task::create(&tx, &payload.task, task_id).await?;

    if let Some(image_ids) = &payload.task.image_ids {
        TaskImage::associate_many_dedup(&tx, task.id, image_ids).await?;
    }

    let project = Project::find_by_id(&tx, task.project_id)
        .await?
        .ok_or(ProjectError::ProjectNotFound)?;

    let agent_working_dir = project
        .default_agent_working_dir
        .as_ref()
        .filter(|dir: &&String| !dir.is_empty())
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

    let workspace_repos: Vec<CreateWorkspaceRepo> = payload
        .repos
        .iter()
        .map(|r| CreateWorkspaceRepo {
            repo_id: r.repo_id,
            target_branch: r.target_branch.clone(),
        })
        .collect();
    WorkspaceRepo::create_many(&tx, workspace.id, &workspace_repos).await?;
    tx.commit().await?;

    if let Err(err) = deployment
        .container()
        .start_workspace(&workspace, payload.executor_profile_id.clone())
        .await
    {
        tracing::error!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            error = %err,
            "Failed to start task attempt"
        );
        if let Err(cleanup_err) =
            cleanup_failed_task_start(&deployment, &task, &workspace).await
        {
            tracing::error!(
                task_id = %task.id,
                workspace_id = %workspace.id,
                error = %cleanup_err,
                "Failed to cleanup task after start failure"
            );
        }
        return Err(ApiError::from(err));
    }

    let task = Task::find_by_id(pool, task.id)
        .await?
        .ok_or(ApiError::Database(DbErr::RecordNotFound(
            "Task not found".to_string(),
        )))?;

    tracing::info!("Started attempt for task {}", task.id);
    Ok(ResponseJson(ApiResponse::success(TaskWithAttemptStatus {
        task,
        has_in_progress_attempt: true,
        last_attempt_failed: false,
        executor: payload.executor_profile_id.executor.to_string(),
    })))
}

async fn cleanup_failed_task_start(
    deployment: &DeploymentImpl,
    task: &Task,
    workspace: &Workspace,
) -> Result<(), ApiError> {
    let pool = &deployment.db().pool;
    let workspace_for_cleanup = Workspace::find_by_id(pool, workspace.id)
        .await?
        .unwrap_or_else(|| workspace.clone());

    if let Err(err) = deployment.container().delete(&workspace_for_cleanup).await {
        tracing::error!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            error = %err,
            "Failed to delete workspace worktree after start failure"
        );
    }

    let rows = Task::delete(pool, task.id).await?;
    if rows == 0 {
        tracing::warn!(
            task_id = %task.id,
            workspace_id = %workspace.id,
            "Task cleanup skipped because task no longer exists"
        );
    }

    Ok(())
}

pub async fn update_task(
    Extension(existing_task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,

    Json(payload): Json<UpdateTask>,
) -> Result<ResponseJson<ApiResponse<Task>>, ApiError> {
    // Use existing values if not provided in update
    let title = payload.title.unwrap_or(existing_task.title);
    let description = match payload.description {
        Some(s) if s.trim().is_empty() => None, // Empty string = clear description
        Some(s) => Some(s),                     // Non-empty string = update description
        None => existing_task.description,      // Field omitted = keep existing
    };
    let status = payload.status.unwrap_or(existing_task.status);
    let parent_workspace_id = payload
        .parent_workspace_id
        .or(existing_task.parent_workspace_id);

    let task = Task::update(
        &deployment.db().pool,
        existing_task.id,
        existing_task.project_id,
        title,
        description,
        status,
        parent_workspace_id,
    )
    .await?;

    if let Some(image_ids) = &payload.image_ids {
        TaskImage::delete_by_task_id(&deployment.db().pool, task.id).await?;
        TaskImage::associate_many_dedup(&deployment.db().pool, task.id, image_ids).await?;
    }

    Ok(ResponseJson(ApiResponse::success(task)))
}

pub async fn delete_task(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<(StatusCode, ResponseJson<ApiResponse<()>>), ApiError> {
    task_deletion::delete_task_with_cleanup(
        &deployment,
        task,
        task_deletion::DeleteTaskMode::CascadeGroup,
    )
    .await?;

    Ok((StatusCode::ACCEPTED, ResponseJson(ApiResponse::success(()))))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let task_actions_router = Router::new()
        .route("/", put(update_task))
        .route("/", delete(delete_task));

    let task_id_router = Router::new()
        .route("/", get(get_task))
        .merge(task_actions_router)
        .layer(from_fn_with_state(deployment.clone(), load_task_middleware));

    let inner = Router::new()
        .route("/", get(get_tasks).post(create_task))
        .route("/stream/ws", get(stream_tasks_ws))
        .route("/create-and-start", post(create_task_and_start))
        .nest("/{task_id}", task_id_router);

    // mount under /projects/:project_id/tasks
    Router::new().nest("/tasks", inner)
}
