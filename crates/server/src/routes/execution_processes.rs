use std::{sync::OnceLock, time::Duration};

use anyhow;
use app_runtime::Deployment;
use axum::{
    Extension, Router,
    extract::{
        Path, Query, State,
        ws::{CloseCode, CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, header},
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessPublic, ExecutionProcessStatus},
    execution_process_repo_state::ExecutionProcessRepoState,
};
use execution::container::ContainerService;
use executors::logs::utils::patch::PatchType;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use logs_axum::SequencedLogMsgAxumExt;
use logs_store::LogEntryEvent;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils_core::{log_entries::LogEntryChannel, response::ApiResponse};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_execution_process_middleware};

const DEFAULT_NORMALIZED_HISTORY_PAGE_SIZE: usize = 20;
const DEFAULT_RAW_HISTORY_PAGE_SIZE: usize = 200;
const MAX_HISTORY_PAGE_SIZE: usize = 1000;
const WS_PING_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
pub struct ExecutionProcessQuery {
    pub workspace_id: Uuid,
    /// If true, include soft-deleted (dropped) processes in results/stream
    #[serde(default)]
    pub show_soft_deleted: Option<bool>,
    pub after_seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct LogHistoryQuery {
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct IndexedLogEntry {
    pub entry_index: i64,
    pub entry: PatchType,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct LogHistoryPage {
    pub entries: Vec<IndexedLogEntry>,
    pub next_cursor: Option<i64>,
    pub has_more: bool,
    pub history_truncated: bool,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogStreamEvent {
    Append { entry_index: i64, entry: PatchType },
    Replace { entry_index: i64, entry: PatchType },
    Finished,
}

struct LogHistoryConfig {
    normalized_page_size: usize,
    raw_page_size: usize,
}

static LOG_HISTORY_CONFIG: OnceLock<LogHistoryConfig> = OnceLock::new();

fn log_history_config() -> &'static LogHistoryConfig {
    LOG_HISTORY_CONFIG.get_or_init(|| {
        let normalized_page_size = read_env_usize(
            "VK_NORMALIZED_LOG_HISTORY_PAGE_SIZE",
            DEFAULT_NORMALIZED_HISTORY_PAGE_SIZE,
        );
        let raw_page_size = read_env_usize(
            "VK_RAW_LOG_HISTORY_PAGE_SIZE",
            DEFAULT_RAW_HISTORY_PAGE_SIZE,
        );

        LogHistoryConfig {
            normalized_page_size: normalized_page_size.max(1),
            raw_page_size: raw_page_size.max(1),
        }
    })
}

fn read_env_usize(name: &str, default: usize) -> usize {
    match std::env::var(name) {
        Ok(value) => match value.parse::<usize>() {
            Ok(parsed) => parsed,
            Err(err) => {
                tracing::warn!("Invalid {name}='{value}': {err}. Using default {default}.");
                default
            }
        },
        Err(_) => default,
    }
}

pub async fn get_execution_process_by_id(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(_deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcessPublic>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(
        ExecutionProcessPublic::from_process(&execution_process),
    )))
}

pub async fn get_raw_logs_v2(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<LogHistoryQuery>,
) -> Result<ResponseJson<ApiResponse<LogHistoryPage>>, ApiError> {
    let page = build_log_history_page(&deployment, &execution_process, LogEntryChannel::Raw, query)
        .await?;
    Ok(ResponseJson(ApiResponse::success(page)))
}

pub async fn get_normalized_logs_v2(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<LogHistoryQuery>,
) -> Result<ResponseJson<ApiResponse<LogHistoryPage>>, ApiError> {
    let page = build_log_history_page(
        &deployment,
        &execution_process,
        LogEntryChannel::Normalized,
        query,
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(page)))
}

async fn build_log_history_page(
    deployment: &DeploymentImpl,
    execution_process: &ExecutionProcess,
    channel: LogEntryChannel,
    query: LogHistoryQuery,
) -> Result<LogHistoryPage, ApiError> {
    let config = log_history_config();
    let default_limit = match channel {
        LogEntryChannel::Raw => config.raw_page_size,
        LogEntryChannel::Normalized => config.normalized_page_size,
    };
    let limit = query
        .limit
        .unwrap_or(default_limit)
        .clamp(1, MAX_HISTORY_PAGE_SIZE);

    let page = deployment
        .container()
        .log_history_page(execution_process, channel, limit, query.cursor)
        .await?;

    let entries = page
        .entries
        .into_iter()
        .filter_map(
            |entry| match serde_json::from_value::<PatchType>(entry.entry_json) {
                Ok(payload) => Some(IndexedLogEntry {
                    entry_index: entry.entry_index as i64,
                    entry: payload,
                }),
                Err(err) => {
                    tracing::warn!(
                        "Failed to decode log entry {} for {}: {}",
                        entry.entry_index,
                        execution_process.id,
                        err
                    );
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    let next_cursor = entries.first().map(|entry| entry.entry_index);

    Ok(LogHistoryPage {
        entries,
        next_cursor,
        has_more: page.has_more,
        history_truncated: page.history_truncated,
    })
}

pub async fn stream_raw_logs_v2_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    ws.on_upgrade(move |socket| async move {
        let shutdown = deployment.shutdown_token();
        let stream = deployment
            .container()
            .stream_raw_log_entries(&exec_id)
            .await;

        let Some(stream) = stream else {
            tracing::warn!(
                execution_process_id = %exec_id,
                user_agent = user_agent.as_deref(),
                close_code = 4404_u16,
                close_reason = "execution_process_not_found",
                "raw logs WS rejected"
            );

            let (mut sender, _) = socket.split();
            let _ = sender
                .send(Message::Close(Some(CloseFrame {
                    code: CloseCode::from(4404_u16),
                    reason: Utf8Bytes::from_static("execution_process_not_found"),
                })))
                .await;
            let _ = sender.close().await;
            return;
        };

        if let Err(e) = handle_log_entries_ws(socket, stream, shutdown).await {
            tracing::warn!("raw logs WS closed: {}", e);
        }
    })
}

pub async fn stream_normalized_logs_v2_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    ws.on_upgrade(move |socket| async move {
        let shutdown = deployment.shutdown_token();
        let stream = deployment
            .container()
            .stream_normalized_log_entries(&exec_id)
            .await;

        let Some(stream) = stream else {
            tracing::warn!(
                execution_process_id = %exec_id,
                user_agent = user_agent.as_deref(),
                close_code = 4404_u16,
                close_reason = "execution_process_not_found",
                "normalized logs WS rejected"
            );

            let (mut sender, _) = socket.split();
            let _ = sender
                .send(Message::Close(Some(CloseFrame {
                    code: CloseCode::from(4404_u16),
                    reason: Utf8Bytes::from_static("execution_process_not_found"),
                })))
                .await;
            let _ = sender.close().await;
            return;
        };

        if let Err(e) = handle_log_entries_ws(socket, stream, shutdown).await {
            tracing::warn!("normalized logs WS closed: {}", e);
        }
    })
}

async fn handle_log_entries_ws(
    socket: WebSocket,
    stream: impl futures_util::Stream<Item = Result<LogEntryEvent, std::io::Error>>
    + Unpin
    + Send
    + 'static,
    shutdown: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut stream = stream;

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
                    Some(Ok(event)) => {
                        if let Some(message) = log_entry_event_to_message(event) {
                            let is_finished = matches!(message.event, LogStreamEvent::Finished);
                            if sender.send(message.ws_message).await.is_err() {
                                break;
                            }
                            if is_finished {
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("log entry stream error: {}", e);
                        break;
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
    Ok(())
}

struct EncodedLogMessage {
    event: LogStreamEvent,
    ws_message: Message,
}

fn log_entry_event_to_message(event: LogEntryEvent) -> Option<EncodedLogMessage> {
    let payload = match event {
        LogEntryEvent::Append { entry_index, entry } => {
            let entry = serde_json::from_value(entry)
                .map_err(|err| {
                    tracing::warn!("Failed to decode append entry: {}", err);
                })
                .ok()?;
            LogStreamEvent::Append {
                entry_index: entry_index as i64,
                entry,
            }
        }
        LogEntryEvent::Replace { entry_index, entry } => {
            let entry = serde_json::from_value(entry)
                .map_err(|err| {
                    tracing::warn!("Failed to decode replace entry: {}", err);
                })
                .ok()?;
            LogStreamEvent::Replace {
                entry_index: entry_index as i64,
                entry,
            }
        }
        LogEntryEvent::Finished => LogStreamEvent::Finished,
    };

    let json = serde_json::to_string(&payload)
        .map_err(|err| tracing::warn!("Failed to serialize log stream event: {}", err))
        .ok()?;

    Some(EncodedLogMessage {
        event: payload,
        ws_message: Message::Text(json.into()),
    })
}

pub async fn stop_execution_process(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    deployment
        .container()
        .stop_execution(&execution_process, ExecutionProcessStatus::Killed)
        .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn stream_execution_processes_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ExecutionProcessQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_execution_processes_ws(
            socket,
            deployment,
            query.workspace_id,
            query.show_soft_deleted.unwrap_or(false),
            query.after_seq,
            user_agent,
        )
        .await
        {
            tracing::warn!(
                workspace_id = %query.workspace_id,
                error = %e,
                "execution processes WS closed"
            );
        }
    })
}

async fn handle_execution_processes_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
    workspace_id: uuid::Uuid,
    show_soft_deleted: bool,
    after_seq: Option<u64>,
    user_agent: Option<String>,
) -> anyhow::Result<()> {
    let shutdown = deployment.shutdown_token();

    let (mut sender, mut receiver) = socket.split();

    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = match deployment
        .events()
        .stream_execution_processes_for_workspace_raw(workspace_id, show_soft_deleted, after_seq)
        .await
    {
        Ok(stream) => stream.map_ok(|msg| msg.to_ws_message_unchecked()),
        Err(err) => {
            let (code, reason) = match &err {
                events::EventError::Database(db::DbErr::RecordNotFound(_)) => {
                    (4404_u16, "workspace_not_found")
                }
                _ => (1011_u16, "stream_error"),
            };

            tracing::warn!(
                workspace_id = %workspace_id,
                show_soft_deleted = show_soft_deleted,
                after_seq = after_seq,
                user_agent = user_agent.as_deref(),
                close_code = code,
                close_reason = reason,
                error = %err,
                "execution processes WS rejected"
            );

            let _ = sender
                .send(Message::Close(Some(CloseFrame {
                    code: CloseCode::from(code),
                    reason: Utf8Bytes::from_static(reason),
                })))
                .await;

            let _ = sender.close().await;
            return Ok(());
        }
    };
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

pub async fn get_execution_process_repo_states(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ExecutionProcessRepoState>>>, ApiError> {
    let pool = &deployment.db().pool;
    let repo_states =
        ExecutionProcessRepoState::find_by_execution_process_id(pool, execution_process.id).await?;
    Ok(ResponseJson(ApiResponse::success(repo_states)))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    // WebSocket routes must not rely on the model-loader middleware: returning a 404 during the
    // WS handshake surfaces as a browser-side 1006 and can trigger aggressive reconnect loops.
    // Instead, WS handlers should always upgrade and then close with a meaningful close code.
    let workspace_id_ws_router = Router::new()
        .route("/raw-logs/v2/ws", get(stream_raw_logs_v2_ws))
        .route("/normalized-logs/v2/ws", get(stream_normalized_logs_v2_ws));

    let workspace_id_http_router = Router::new()
        .route("/", get(get_execution_process_by_id))
        .route("/stop", post(stop_execution_process))
        .route("/repo-states", get(get_execution_process_repo_states))
        .route("/raw-logs/v2", get(get_raw_logs_v2))
        .route("/normalized-logs/v2", get(get_normalized_logs_v2))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_execution_process_middleware::<DeploymentImpl>,
        ));

    let workspace_id_router = workspace_id_ws_router.merge(workspace_id_http_router);

    let workspaces_router = Router::new()
        .route("/stream/ws", get(stream_execution_processes_ws))
        .nest("/{id}", workspace_id_router);

    Router::new().nest("/execution-processes", workspaces_router)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use app_runtime::Deployment;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use db::models::{
        execution_process::{CreateExecutionProcess, ExecutionProcessRunReason},
        project::{CreateProject, Project},
        session::{CreateSession, Session},
        task::{CreateTask, Task},
        workspace::{CreateWorkspace, Workspace},
    };
    use executors_protocol::actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{DeploymentImpl, http, test_support::TestEnvGuard};

    #[tokio::test]
    async fn execution_process_api_does_not_expose_script_contents() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let _env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();
        let pool = &deployment.db().pool;

        let project_id = Uuid::new_v4();
        Project::create(
            pool,
            &CreateProject {
                name: "Redaction".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            pool,
            &CreateTask::from_title_description(project_id, "T".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let workspace_id = Uuid::new_v4();
        Workspace::create(
            pool,
            &CreateWorkspace {
                branch: "main".to_string(),
                agent_working_dir: None,
            },
            workspace_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        let session = Session::create(
            pool,
            &CreateSession {
                executor: Some("test".to_string()),
            },
            session_id,
            workspace_id,
        )
        .await
        .unwrap();

        let secret = "sekrit-value-123";
        let process_id = Uuid::new_v4();
        db::models::execution_process::ExecutionProcess::create(
            pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: ExecutorAction::new(
                    ExecutorActionType::ScriptRequest(ScriptRequest {
                        script: format!("echo {secret}"),
                        language: ScriptRequestLanguage::Bash,
                        context: ScriptContext::SetupScript,
                        working_dir: None,
                    }),
                    None,
                ),
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            process_id,
            &[],
        )
        .await
        .unwrap();

        let app = http::router(deployment);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/execution-processes/{process_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8_lossy(&body);
        assert!(!body_str.contains(secret));

        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.pointer("/data/executor_action/typ/type")
                .and_then(|v| v.as_str()),
            Some("ScriptRequest")
        );
        assert_eq!(
            json.pointer("/data/executor_action/typ/script")
                .and_then(|v| v.as_str()),
            Some("<redacted>")
        );
    }
}
