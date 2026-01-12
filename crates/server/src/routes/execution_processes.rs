use std::sync::OnceLock;

use anyhow;
use axum::{
    Extension, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessError, ExecutionProcessStatus},
    execution_process_repo_state::ExecutionProcessRepoState,
};
use deployment::Deployment;
use executors::logs::utils::patch::PatchType;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::{log_entries::LogEntryChannel, msg_store::LogEntryEvent, response::ApiResponse};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_execution_process_middleware};

const DEFAULT_NORMALIZED_HISTORY_PAGE_SIZE: usize = 20;
const DEFAULT_RAW_HISTORY_PAGE_SIZE: usize = 200;
const MAX_HISTORY_PAGE_SIZE: usize = 1000;

#[derive(Debug, Deserialize)]
pub struct ExecutionProcessQuery {
    pub workspace_id: Uuid,
    /// If true, include soft-deleted (dropped) processes in results/stream
    #[serde(default)]
    pub show_soft_deleted: Option<bool>,
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
) -> Result<ResponseJson<ApiResponse<ExecutionProcess>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(execution_process)))
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
) -> Result<impl IntoResponse, ApiError> {
    let stream = deployment
        .container()
        .stream_raw_log_entries(&exec_id)
        .await
        .ok_or_else(|| {
            ApiError::ExecutionProcess(ExecutionProcessError::ExecutionProcessNotFound)
        })?;

    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_log_entries_ws(socket, stream).await {
            tracing::warn!("raw logs WS closed: {}", e);
        }
    }))
}

pub async fn stream_normalized_logs_v2_ws(
    ws: WebSocketUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let stream = deployment
        .container()
        .stream_normalized_log_entries(&exec_id)
        .await
        .ok_or_else(|| {
            ApiError::ExecutionProcess(ExecutionProcessError::ExecutionProcessNotFound)
        })?;

    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_log_entries_ws(socket, stream).await {
            tracing::warn!("normalized logs WS closed: {}", e);
        }
    }))
}

async fn handle_log_entries_ws(
    socket: WebSocket,
    stream: impl futures_util::Stream<Item = Result<LogEntryEvent, std::io::Error>>
    + Unpin
    + Send
    + 'static,
) -> anyhow::Result<()> {
    let mut stream = stream;

    let (mut sender, mut receiver) = socket.split();
    tokio::spawn(async move { while let Some(Ok(_)) = receiver.next().await {} });

    while let Some(item) = stream.next().await {
        match item {
            Ok(event) => {
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
            Err(e) => {
                tracing::error!("log entry stream error: {}", e);
                break;
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
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_execution_processes_ws(
            socket,
            deployment,
            query.workspace_id,
            query.show_soft_deleted.unwrap_or(false),
        )
        .await
        {
            tracing::warn!("execution processes WS closed: {}", e);
        }
    })
}

async fn handle_execution_processes_ws(
    socket: WebSocket,
    deployment: DeploymentImpl,
    workspace_id: uuid::Uuid,
    show_soft_deleted: bool,
) -> anyhow::Result<()> {
    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = deployment
        .events()
        .stream_execution_processes_for_workspace_raw(workspace_id, show_soft_deleted)
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
    let workspace_id_router = Router::new()
        .route("/", get(get_execution_process_by_id))
        .route("/stop", post(stop_execution_process))
        .route("/repo-states", get(get_execution_process_repo_states))
        .route("/raw-logs/v2", get(get_raw_logs_v2))
        .route("/raw-logs/v2/ws", get(stream_raw_logs_v2_ws))
        .route("/normalized-logs/v2", get(get_normalized_logs_v2))
        .route("/normalized-logs/v2/ws", get(stream_normalized_logs_v2_ws))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_execution_process_middleware,
        ));

    let workspaces_router = Router::new()
        .route("/stream/ws", get(stream_execution_processes_ws))
        .nest("/{id}", workspace_id_router);

    Router::new().nest("/execution-processes", workspaces_router)
}
