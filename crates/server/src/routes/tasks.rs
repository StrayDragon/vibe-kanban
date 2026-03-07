use anyhow;
use app_runtime::Deployment;
use axum::{
    Extension, Json, Router,
    extract::{
        Query, State,
        ws::{WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{delete, get, post, put},
};
use db::models::{
    image::TaskImage,
    task::{CreateTask, Task, TaskLineageSummary, TaskWithAttemptStatus, UpdateTask},
    workspace_repo::CreateWorkspaceRepo,
};
use executors_protocol::ExecutorProfileId;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use logs_axum::LogMsgAxumExt;
use serde::{Deserialize, Serialize};
use tasks::orchestration::{self, CreateAndStartTaskInput};
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::load_task_middleware,
    routes::{task_attempts::WorkspaceRepoInput, task_deletion},
    task_runtime::DeploymentTaskRuntime,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskQuery {
    pub project_id: Option<Uuid>,
    pub include_archived: Option<bool>,
    pub archived_kanban_id: Option<Uuid>,
}

pub async fn get_tasks(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskWithAttemptStatus>>>, ApiError> {
    let include_archived = query.include_archived.unwrap_or(false);
    let tasks = Task::find_filtered_with_attempt_status(
        &deployment.db().pool,
        query.project_id,
        include_archived,
        query.archived_kanban_id,
    )
    .await?;

    Ok(ResponseJson(ApiResponse::success(tasks)))
}

pub async fn stream_tasks_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TaskQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_tasks_ws(socket, deployment, query).await {
            tracing::warn!("tasks WS closed: {}", e);
        }
    })
}

async fn handle_tasks_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
    query: TaskQuery,
) -> anyhow::Result<()> {
    let shutdown = deployment.shutdown_token();
    let include_archived = query.include_archived.unwrap_or(false);
    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = deployment
        .events()
        .stream_tasks_raw(query.project_id, include_archived, query.archived_kanban_id)
        .await?
        .map_ok(|msg| msg.to_ws_message_unchecked());

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
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

pub async fn get_task(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TaskWithAttemptStatus>>, ApiError> {
    let task = Task::find_by_id_with_attempt_status(&deployment.db().pool, task.id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Task not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(task)))
}

pub async fn get_task_lineage(
    Extension(task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TaskLineageSummary>>, ApiError> {
    let lineage = Task::lineage_summary(&deployment.db().pool, task.id).await?;
    Ok(ResponseJson(ApiResponse::success(lineage)))
}

pub async fn create_task(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    Json(payload): Json<CreateTask>,
) -> Result<ResponseJson<ApiResponse<Task>>, ApiError> {
    let key = crate::routes::idempotency::idempotency_key(&headers);
    let hash = crate::routes::idempotency::request_hash(&payload)?;

    crate::routes::idempotency::idempotent_success(
        &deployment.db().pool,
        "create_task",
        key,
        hash,
        || async {
            orchestration::create_task(&deployment.db().pool, &payload)
                .await
                .map_err(ApiError::from)
        },
    )
    .await
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
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    let repos: Vec<CreateWorkspaceRepo> = payload
        .repos
        .iter()
        .map(|repo| CreateWorkspaceRepo {
            repo_id: repo.repo_id,
            target_branch: repo.target_branch.clone(),
        })
        .collect();

    let task = orchestration::create_task_and_start(
        &runtime,
        &deployment.db().pool,
        &CreateAndStartTaskInput {
            task: payload.task,
            executor_profile_id: payload.executor_profile_id,
            repos,
        },
    )
    .await?;

    Ok(ResponseJson(ApiResponse::success(task)))
}

pub async fn update_task(
    Extension(existing_task): Extension<Task>,
    State(deployment): State<DeploymentImpl>,

    Json(payload): Json<UpdateTask>,
) -> Result<ResponseJson<ApiResponse<Task>>, ApiError> {
    if existing_task.archived_kanban_id.is_some() {
        return Err(ApiError::Conflict(
            "Task is archived. Restore it before editing.".to_string(),
        ));
    }
    // Use existing values if not provided in update
    let title = payload.title.unwrap_or(existing_task.title);
    let description = match payload.description {
        Some(s) if s.trim().is_empty() => None, // Empty string = clear description
        Some(s) => Some(s),                     // Non-empty string = update description
        None => existing_task.description,      // Field omitted = keep existing
    };
    let status = payload.status.unwrap_or(existing_task.status);
    let automation_mode = payload
        .automation_mode
        .unwrap_or(existing_task.automation_mode);
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
        automation_mode,
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
    if task.archived_kanban_id.is_some() {
        return Err(ApiError::Conflict(
            "Task is archived. Delete its archive to remove it.".to_string(),
        ));
    }
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
        .route("/lineage", get(get_task_lineage))
        .merge(task_actions_router)
        .layer(from_fn_with_state(
            deployment.clone(),
            load_task_middleware::<DeploymentImpl>,
        ));

    let inner = Router::new()
        .route("/", get(get_tasks).post(create_task))
        .route("/stream/ws", get(stream_tasks_ws))
        .route("/create-and-start", post(create_task_and_start))
        .nest("/{task_id}", task_id_router);

    // mount under /projects/:project_id/tasks
    Router::new().nest("/tasks", inner)
}

#[cfg(test)]
mod tests {
    use app_runtime::Deployment;
    use axum::{Extension, Json, extract::State, http::HeaderValue};
    use db::models::{
        project::{CreateProject, Project},
        task::{CreateTask, Task},
    };
    use uuid::Uuid;

    use super::{create_task, get_task_lineage};
    use crate::{DeploymentImpl, test_support::TestEnvGuard};

    fn idempotency_headers(key: &'static str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Idempotency-Key", HeaderValue::from_static(key));
        headers
    }

    #[tokio::test]
    async fn create_task_is_idempotent_by_idempotency_key() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);
        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Idempotency".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let payload =
            CreateTask::from_title_description(project_id, "A".to_string(), Some("d".to_string()));

        let response1 = create_task(
            State(deployment.clone()),
            idempotency_headers("req-1"),
            Json(payload.clone()),
        )
        .await
        .unwrap();
        let task1 = response1.0.into_data().expect("task should be present");

        let response2 = create_task(
            State(deployment.clone()),
            idempotency_headers("req-1"),
            Json(payload.clone()),
        )
        .await
        .unwrap();
        let task2 = response2.0.into_data().expect("task should be present");

        assert_eq!(task1.id, task2.id);

        let tasks = Task::find_by_project_id_with_attempt_status(&deployment.db().pool, project_id)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn get_task_lineage_returns_origin_and_follow_up_context() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);
        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Lineage".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let origin_task_id = Uuid::new_v4();
        let origin_task = Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(
                project_id,
                "Origin task".to_string(),
                Some("parent".to_string()),
            ),
            origin_task_id,
        )
        .await
        .unwrap();

        let mut follow_up_payload =
            CreateTask::from_title_description(project_id, "Follow-up task".to_string(), None);
        follow_up_payload.origin_task_id = Some(origin_task_id);
        follow_up_payload.created_by_kind = Some(db::types::TaskCreatedByKind::AgentFollowup);

        let follow_up_task =
            Task::create(&deployment.db().pool, &follow_up_payload, Uuid::new_v4())
                .await
                .unwrap();

        let Json(origin_response) =
            get_task_lineage(Extension(origin_task.clone()), State(deployment.clone()))
                .await
                .unwrap();
        let origin_lineage = origin_response.into_data().expect("origin lineage");
        assert!(origin_lineage.origin_task.is_none());
        assert_eq!(origin_lineage.follow_up_tasks.len(), 1);
        assert_eq!(origin_lineage.follow_up_tasks[0].id, follow_up_task.id);
        assert_eq!(
            origin_lineage.follow_up_tasks[0].created_by_kind,
            db::types::TaskCreatedByKind::AgentFollowup
        );

        let Json(follow_up_response) =
            get_task_lineage(Extension(follow_up_task.clone()), State(deployment))
                .await
                .unwrap();
        let follow_up_lineage = follow_up_response.into_data().expect("follow-up lineage");
        assert_eq!(
            follow_up_lineage.origin_task.expect("origin task").id,
            origin_task.id
        );
        assert!(follow_up_lineage.follow_up_tasks.is_empty());
    }

    #[tokio::test]
    async fn create_task_rejects_idempotency_key_reuse_with_different_payload() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);
        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Idempotency conflict".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let payload1 = CreateTask::from_title_description(project_id, "A".to_string(), None);
        let payload2 = CreateTask::from_title_description(project_id, "B".to_string(), None);

        let _ = create_task(
            State(deployment.clone()),
            idempotency_headers("req-1"),
            Json(payload1),
        )
        .await
        .unwrap();

        let err = create_task(
            State(deployment.clone()),
            idempotency_headers("req-1"),
            Json(payload2),
        )
        .await
        .expect_err("expected conflict");

        assert!(matches!(err, crate::error::ApiError::Conflict(_)));
    }
}
