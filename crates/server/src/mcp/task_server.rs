use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use app_runtime::Deployment;
use db::{
    DbErr,
    events::{
        EVENT_EXECUTION_PROCESS_CREATED, EVENT_EXECUTION_PROCESS_DELETED,
        EVENT_EXECUTION_PROCESS_UPDATED, EVENT_PROJECT_CREATED, EVENT_PROJECT_DELETED,
        EVENT_PROJECT_UPDATED, EVENT_TASK_CREATED, EVENT_TASK_DELETED,
        EVENT_TASK_ORCHESTRATION_TRANSITION, EVENT_TASK_UPDATED, EVENT_WORKSPACE_CREATED,
        EVENT_WORKSPACE_DELETED, EVENT_WORKSPACE_UPDATED, ExecutionProcessEventPayload,
        ProjectEventPayload, TaskEventPayload, TaskOrchestrationTransitionEventPayload,
        WorkspaceEventPayload,
    },
    models::{
        approval as approval_model,
        archived_kanban::{ArchivedKanban, ArchivedKanbanWithTaskCount},
        attempt_control_lease as attempt_control_lease_model,
        coding_agent_turn::CodingAgentTurn,
        event_outbox::{EventOutbox, EventOutboxEntry},
        execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
        execution_process_repo_state::CreateExecutionProcessRepoState,
        mcp_tool_task as mcp_tool_task_model,
        project::Project,
        project_repo::ProjectRepo,
        session::Session,
        tag::Tag,
        task::{CreateTask, Task, TaskStatus, TaskWithAttemptStatus},
        workspace::{CreateWorkspace, Workspace},
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    },
};
use execution::container::ContainerService;
use executors_protocol::{
    BaseCodingAgent, ExecutorProfileId,
    actions::{
        ExecutorAction, ExecutorActionType, coding_agent_follow_up::CodingAgentFollowUpRequest,
        coding_agent_initial::CodingAgentInitialRequest,
    },
};
use regex::Regex;
use rmcp::{
    ErrorData, Json, ServerHandler,
    handler::server::tool::ToolRouter,
    model::{
        CallToolRequestParams, CallToolResult, CancelTaskParams, CancelTaskResult, Content,
        CreateTaskResult, EnumSchema, GetTaskInfoParams, GetTaskPayloadResult, GetTaskResult,
        GetTaskResultParams, Icon, Implementation, InitializeRequestParams, PaginatedRequestParams,
        ProtocolVersion, ServerCapabilities, ServerInfo, Task as McpProtocolTask,
        TaskStatus as McpProtocolTaskStatus, TasksCapability,
    },
    schemars, tool_handler,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, mcp::params::Parameters};

mod errors;
mod params;
mod runtime;
mod tools;

use errors::*;
pub use params::*;
use runtime::*;

const TAIL_ATTEMPT_FEED_MAX_WAIT_MS: u64 = 30_000;

const DEFAULT_IDEMPOTENCY_IN_PROGRESS_TTL_SECS: i64 = 60 * 60;
const IDEMPOTENCY_IN_PROGRESS_TTL_ENV: &str = "VK_IDEMPOTENCY_IN_PROGRESS_TTL_SECS";

const DEFAULT_ATTEMPT_CONTROL_LEASE_TTL_SECS: i64 = 60 * 60;
const ATTEMPT_CONTROL_LEASE_MAX_TTL_SECS: i64 = 24 * 60 * 60;

const DEFAULT_MCP_TASK_TTL_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const MCP_TASK_TTL_MS_ENV: &str = "VK_MCP_TASK_TTL_MS";

const DEFAULT_MCP_TASK_POLL_INTERVAL_MS: u64 = 1_000;
const MCP_TASK_POLL_INTERVAL_MS_ENV: &str = "VK_MCP_TASK_POLL_INTERVAL_MS";

const DEFAULT_MCP_TASK_MAX_CONCURRENCY: usize = 4;
const MCP_TASK_MAX_CONCURRENCY_ENV: &str = "VK_MCP_TASK_MAX_CONCURRENCY";

fn tool_output_schema<T: schemars::JsonSchema + 'static>() -> Arc<Map<String, Value>> {
    rmcp::handler::server::tool::schema_for_output::<T>().unwrap_or_else(|e| {
        panic!(
            "Invalid output schema for {}: {}",
            std::any::type_name::<T>(),
            e
        )
    })
}

#[derive(Clone)]
pub struct TaskServer {
    deployment: DeploymentImpl,
    tool_router: ToolRouter<TaskServer>,
    peer: Arc<std::sync::RwLock<Option<rmcp::service::Peer<rmcp::RoleServer>>>>,
    approvals_elicitation_started: Arc<AtomicBool>,
    mcp_tasks: Arc<McpTasksRuntime>,
}

struct RunningMcpTask {
    ct: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

struct McpTasksRuntime {
    running: tokio::sync::Mutex<HashMap<String, RunningMcpTask>>,
    semaphore: Arc<tokio::sync::Semaphore>,
    poll_interval_ms: u64,
    ttl_ms: Option<u64>,
    resumer_started: AtomicBool,
}

impl TaskServer {
    pub fn new(deployment: DeploymentImpl) -> Self {
        let runtime = Arc::new(McpTasksRuntime {
            running: tokio::sync::Mutex::new(HashMap::new()),
            semaphore: Arc::new(tokio::sync::Semaphore::new(mcp_task_max_concurrency())),
            poll_interval_ms: mcp_task_poll_interval_ms(),
            ttl_ms: mcp_task_ttl_ms(),
            resumer_started: AtomicBool::new(false),
        });

        Self {
            deployment,
            tool_router: tools::build_tool_router(),
            peer: Arc::new(std::sync::RwLock::new(None)),
            approvals_elicitation_started: Arc::new(AtomicBool::new(false)),
            mcp_tasks: runtime,
        }
    }

    fn record_peer(&self, peer: rmcp::service::Peer<rmcp::RoleServer>) {
        if let Ok(mut guard) = self.peer.write() {
            *guard = Some(peer);
        }
    }

    fn start_approvals_elicitation_if_supported(
        &self,
        peer: rmcp::service::Peer<rmcp::RoleServer>,
    ) {
        if !peer
            .supported_elicitation_modes()
            .contains(&rmcp::service::ElicitationMode::Form)
        {
            return;
        }

        if self
            .approvals_elicitation_started
            .swap(true, Ordering::SeqCst)
        {
            return;
        }

        let approvals = self.deployment.approvals().clone();
        let pool = self.deployment.db().pool.clone();

        let responded_by_client_id = peer
            .peer_info()
            .map(|info| format!("mcp:{}@{}", info.client_info.name, info.client_info.version));

        let mut rx = approvals.subscribe_created();
        tokio::spawn(async move {
            loop {
                let approval = match rx.recv().await {
                    Ok(approval) => approval,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };

                let approval_uuid = match Uuid::parse_str(&approval.id) {
                    Ok(uuid) => uuid,
                    Err(err) => {
                        tracing::warn!(
                            approval_id = approval.id,
                            error = %err,
                            "Skipping elicitation for approval with invalid id"
                        );
                        continue;
                    }
                };

                let current = match approval_model::get_by_id(&pool, approval_uuid).await {
                    Ok(Some(approval)) => approval,
                    Ok(None) => continue,
                    Err(err) => {
                        tracing::warn!(
                            approval_id = approval.id,
                            error = %err,
                            "Failed to load approval while attempting elicitation"
                        );
                        continue;
                    }
                };

                if !matches!(
                    current.status,
                    utils_core::approvals::ApprovalStatus::Pending
                ) {
                    continue;
                }

                let input_pretty = serde_json::to_string_pretty(&approval.tool_input)
                    .unwrap_or_else(|_| approval.tool_input.to_string());

                let message = format!(
                    "Approval needed.\n\napproval_id: {}\nexecution_process_id: {}\ntool: {}\n\ntool_input:\n{}",
                    approval.id, approval.execution_process_id, approval.tool_name, input_pretty
                );

                let timeout = (approval.timeout_at - chrono::Utc::now()).to_std().ok();

                let decision_schema =
                    EnumSchema::builder(vec!["approved".to_string(), "denied".to_string()]).build();

                let schema = match rmcp::model::ElicitationSchema::builder()
                    .required_enum_schema("decision", decision_schema)
                    .optional_string_with("denial_reason", |s| {
                        s.description("Optional denial reason (used when decision=denied)")
                    })
                    .description("Approval response")
                    .build()
                {
                    Ok(schema) => schema,
                    Err(err) => {
                        tracing::warn!(
                            approval_id = approval.id,
                            error = err,
                            "Failed to build elicitation schema; skipping approval elicitation"
                        );
                        continue;
                    }
                };

                let elicitation = match peer
                    .create_elicitation_with_timeout(
                        rmcp::model::CreateElicitationRequestParams::FormElicitationParams {
                            meta: None,
                            message,
                            requested_schema: schema,
                        },
                        timeout,
                    )
                    .await
                {
                    Ok(result) => result,
                    Err(rmcp::service::ServiceError::Timeout { .. }) => continue,
                    Err(err) => {
                        tracing::warn!(
                            approval_id = approval.id,
                            error = %err,
                            "Approval elicitation failed"
                        );
                        continue;
                    }
                };

                let content = match elicitation.action {
                    rmcp::model::ElicitationAction::Accept => {
                        let Some(content) = elicitation.content else {
                            continue;
                        };
                        content
                    }
                    rmcp::model::ElicitationAction::Decline
                    | rmcp::model::ElicitationAction::Cancel => {
                        continue;
                    }
                };

                let (decision, denial_reason) = match content {
                    Value::Object(map) => (
                        map.get("decision")
                            .and_then(|value| value.as_str())
                            .map(|s| s.to_string()),
                        map.get("denial_reason")
                            .and_then(|value| value.as_str())
                            .map(|s| s.to_string()),
                    ),
                    _ => (None, None),
                };

                let decision = match decision.as_deref() {
                    Some("approved") => "approved",
                    Some("denied") => "denied",
                    Some(other) => {
                        tracing::warn!(
                            approval_id = approval.id,
                            decision = other,
                            "Unknown approval decision from elicitation; skipping"
                        );
                        continue;
                    }
                    None => {
                        tracing::warn!(
                            approval_id = approval.id,
                            "Missing decision in approval elicitation response; skipping"
                        );
                        continue;
                    }
                };

                let status = match decision {
                    "approved" => utils_core::approvals::ApprovalStatus::Approved,
                    "denied" => utils_core::approvals::ApprovalStatus::Denied {
                        reason: denial_reason,
                    },
                    _ => utils_core::approvals::ApprovalStatus::Denied {
                        reason: Some("invalid decision".to_string()),
                    },
                };

                let response = utils_core::approvals::ApprovalResponse {
                    execution_process_id: approval.execution_process_id,
                    status,
                };

                if let Err(err) = approvals
                    .respond_with_client_id(
                        &pool,
                        &approval.id,
                        response,
                        responded_by_client_id.clone(),
                    )
                    .await
                {
                    tracing::warn!(
                        approval_id = approval.id,
                        error = %err,
                        "Failed to apply approval response from elicitation"
                    );
                }
            }
        });
    }

    fn spawn_mcp_task_resumer(&self, peer: rmcp::service::Peer<rmcp::RoleServer>) {
        if self.mcp_tasks.resumer_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let server = self.clone();
        tokio::spawn(async move {
            if let Err(err) = server.resume_working_mcp_tasks(peer).await {
                tracing::warn!(error = %err, "Failed to resume MCP tasks");
            }
        });
    }

    async fn resume_working_mcp_tasks(
        &self,
        peer: rmcp::service::Peer<rmcp::RoleServer>,
    ) -> Result<(), DbErr> {
        let pool = &self.deployment.db().pool;
        let _ = mcp_tool_task_model::delete_expired(pool).await;

        let tasks = mcp_tool_task_model::list_working(pool).await?;
        for task in tasks {
            if task.resumable {
                self.spawn_mcp_tool_task_execution(
                    task.task_id.clone(),
                    task.tool_name.clone(),
                    task.tool_arguments_json.clone(),
                    peer.clone(),
                )
                .await;
            } else {
                let _ = mcp_tool_task_model::update_status(
                    pool,
                    &task.task_id,
                    "failed",
                    Some("Server restarted; task was not resumable.".to_string()),
                )
                .await;
            }
        }

        Ok(())
    }

    async fn spawn_mcp_tool_task_execution(
        &self,
        task_id: String,
        tool_name: String,
        tool_arguments_json: serde_json::Value,
        peer: rmcp::service::Peer<rmcp::RoleServer>,
    ) {
        {
            let running = self.mcp_tasks.running.lock().await;
            if running.contains_key(&task_id) {
                return;
            }
        }

        let ct = CancellationToken::new();
        let ct_for_exec = ct.clone();
        let task_id_for_exec = task_id.clone();
        let server = self.clone();
        let runtime = self.mcp_tasks.clone();
        let handle = tokio::spawn(async move {
            let _permit = runtime
                .semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("MCP task semaphore closed");

            let pool = &server.deployment.db().pool;
            let current = match mcp_tool_task_model::find_by_task_id(pool, &task_id_for_exec).await
            {
                Ok(Some(task)) => task,
                Ok(None) => return,
                Err(err) => {
                    tracing::warn!(task_id = task_id_for_exec, error = %err, "Failed to load MCP task");
                    return;
                }
            };

            if current.status != "working" {
                let mut running = runtime.running.lock().await;
                running.remove(&task_id_for_exec);
                return;
            }

            let arguments = tool_arguments_json.as_object().cloned();
            let request = CallToolRequestParams {
                meta: None,
                name: std::borrow::Cow::Owned(tool_name.clone()),
                arguments,
                task: None,
            };

            let context = rmcp::service::RequestContext::<rmcp::RoleServer> {
                ct: ct_for_exec.clone(),
                id: rmcp::model::NumberOrString::String(format!("task:{task_id_for_exec}").into()),
                meta: rmcp::model::Meta::new(),
                extensions: rmcp::model::Extensions::default(),
                peer,
            };

            let execution = tokio::select! {
                _ = ct_for_exec.cancelled() => None,
                result = server.call_tool(request, context) => Some(result),
            };

            match execution {
                None => {
                    let _ = mcp_tool_task_model::update_status(
                        pool,
                        &task_id_for_exec,
                        "cancelled",
                        Some("Cancelled.".to_string()),
                    )
                    .await;
                }
                Some(Ok(result)) => {
                    let status = if result.is_error == Some(true) {
                        "failed"
                    } else {
                        "completed"
                    };

                    let payload = serde_json::to_value(&result).unwrap_or_else(|_| {
                        json!({
                            "content": [{"type": "text", "text": "Failed to serialize tool result."}],
                            "isError": true
                        })
                    });

                    let _ = mcp_tool_task_model::finish_with_payload(
                        pool,
                        &task_id_for_exec,
                        status,
                        payload,
                        None,
                    )
                    .await;
                }
                Some(Err(err)) => {
                    let tool_result = TaskServer::err_with(
                        "Tool execution failed.",
                        Some(json!({
                            "task_id": task_id_for_exec,
                            "tool": tool_name,
                            "rpc_error": {
                                "code": err.code.0,
                                "message": err.message,
                                "data": err.data,
                            }
                        })),
                        Some("RPC-level failure while executing tool.".to_string()),
                        Some("rpc_error"),
                        Some(false),
                    )
                    .unwrap_or(CallToolResult {
                        content: vec![Content::text("Tool execution failed.".to_string())],
                        structured_content: None,
                        is_error: Some(true),
                        meta: None,
                    });

                    let payload = serde_json::to_value(&tool_result).unwrap_or_else(|_| {
                        json!({
                            "content": [{"type": "text", "text": "Tool execution failed."}],
                            "isError": true
                        })
                    });

                    let _ = mcp_tool_task_model::finish_with_payload(
                        pool,
                        &task_id_for_exec,
                        "failed",
                        payload,
                        Some("Tool execution failed.".to_string()),
                    )
                    .await;
                }
            }

            let mut running = runtime.running.lock().await;
            running.remove(&task_id_for_exec);
        });

        let mut running = self.mcp_tasks.running.lock().await;
        running.insert(task_id, RunningMcpTask { ct, handle });
    }

    fn mcp_task_status_from_db(raw: &str) -> McpProtocolTaskStatus {
        match raw {
            "working" => McpProtocolTaskStatus::Working,
            "input_required" => McpProtocolTaskStatus::InputRequired,
            "completed" => McpProtocolTaskStatus::Completed,
            "failed" => McpProtocolTaskStatus::Failed,
            "cancelled" => McpProtocolTaskStatus::Cancelled,
            _ => McpProtocolTaskStatus::Working,
        }
    }

    fn mcp_protocol_task_from_record(record: &mcp_tool_task_model::McpToolTask) -> McpProtocolTask {
        McpProtocolTask {
            task_id: record.task_id.clone(),
            status: Self::mcp_task_status_from_db(&record.status),
            status_message: record.status_message.clone(),
            created_at: record.created_at.to_rfc3339(),
            last_updated_at: record.last_updated_at.to_rfc3339(),
            ttl: record.ttl_ms.and_then(|value| u64::try_from(value).ok()),
            poll_interval: record
                .poll_interval_ms
                .and_then(|value| u64::try_from(value).ok()),
        }
    }

    fn stable_tool_idempotency_key(raw: Option<String>) -> Option<String> {
        raw.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
    }

    fn request_hash<T: Serialize>(payload: &T) -> Result<String, ErrorData> {
        let bytes = serde_json::to_vec(payload).map_err(|e| {
            ErrorData::internal_error(
                "Failed to serialize payload for hashing",
                Some(json!({ "error": e.to_string() })),
            )
        })?;
        let digest = Sha256::digest(&bytes);
        Ok(format!("{digest:x}"))
    }

    async fn idempotent<T, F, Fut>(
        &self,
        scope: &'static str,
        key: Option<String>,
        request_hash: String,
        execute: F,
    ) -> Result<T, ToolOrRpcError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, ErrorData>>,
    {
        let Some(key) = key else {
            return execute().await.map_err(ToolOrRpcError::Rpc);
        };

        match db::models::idempotency::begin(
            &self.deployment.db().pool,
            scope,
            &key,
            &request_hash,
            idempotency_in_progress_ttl(),
        )
        .await
        {
            Ok(db::models::idempotency::IdempotencyBeginOutcome::New { record_uuid }) => {
                let result = execute().await;
                match result {
                    Ok(data) => {
                        let response_json = serde_json::to_string(&data).map_err(|e| {
                            ToolOrRpcError::Rpc(ErrorData::internal_error(
                                "Failed to serialize idempotent tool response",
                                Some(json!({ "error": e.to_string(), "scope": scope })),
                            ))
                        })?;
                        if let Err(err) = db::models::idempotency::complete(
                            &self.deployment.db().pool,
                            record_uuid,
                            200,
                            response_json,
                        )
                        .await
                        {
                            tracing::warn!(
                                record_uuid = %record_uuid,
                                error = %err,
                                "Failed to complete idempotency record"
                            );
                        }
                        Ok(data)
                    }
                    Err(err) => {
                        let _ = db::models::idempotency::delete(
                            &self.deployment.db().pool,
                            record_uuid,
                        )
                        .await;
                        Err(ToolOrRpcError::Rpc(err))
                    }
                }
            }
            Ok(db::models::idempotency::IdempotencyBeginOutcome::Existing { record }) => {
                if record.request_hash != request_hash {
                    return Err(ToolOrRpcError::Tool(Self::structured_error(
                        Self::err_payload(
                            "Idempotency key already used with different request parameters",
                            Some(json!({
                                "scope": scope,
                                "request_id": key,
                                "existing_request_hash": record.request_hash,
                                "request_hash": request_hash,
                            })),
                            Some("Use a new request_id for different parameters.".to_string()),
                            Some(MCP_CODE_IDEMPOTENCY_CONFLICT),
                            Some(false),
                        ),
                    )));
                }

                match record.state.as_str() {
                    db::models::idempotency::IDEMPOTENCY_STATE_COMPLETED => {
                        let Some(response_json) = record.response_json else {
                            return Err(ToolOrRpcError::Rpc(ErrorData::internal_error(
                                "Idempotency record completed but missing stored response",
                                Some(json!({ "scope": scope, "request_id": key })),
                            )));
                        };
                        let parsed: T = serde_json::from_str(&response_json).map_err(|e| {
                            ToolOrRpcError::Rpc(ErrorData::internal_error(
                                "Failed to parse stored idempotent response",
                                Some(json!({
                                    "error": e.to_string(),
                                    "scope": scope,
                                    "request_id": key,
                                })),
                            ))
                        })?;
                        Ok(parsed)
                    }
                    db::models::idempotency::IDEMPOTENCY_STATE_IN_PROGRESS => Err(
                        ToolOrRpcError::Tool(Self::structured_error(Self::err_payload(
                            "Request with this idempotency key is in progress.",
                            Some(json!({ "scope": scope, "request_id": key })),
                            Some("Wait briefly and retry the same tool call.".to_string()),
                            Some(MCP_CODE_IDEMPOTENCY_IN_PROGRESS),
                            Some(true),
                        ))),
                    ),
                    other => Err(ToolOrRpcError::Rpc(ErrorData::internal_error(
                        "Unknown idempotency record state",
                        Some(json!({ "state": other, "scope": scope, "request_id": key })),
                    ))),
                }
            }
            Err(err) => Err(ToolOrRpcError::Rpc(ErrorData::internal_error(
                "Idempotency error",
                Some(json!({ "error": err.to_string(), "scope": scope })),
            ))),
        }
    }

    async fn expand_tags(&self, text: &str) -> String {
        let tag_pattern = match Regex::new(r"@([^\s@]+)") {
            Ok(re) => re,
            Err(_) => return text.to_string(),
        };

        let tag_names: Vec<String> = tag_pattern
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if tag_names.is_empty() {
            return text.to_string();
        }

        let tags: Vec<Tag> = match Tag::find_all(&self.deployment.db().pool).await {
            Ok(tags) => tags,
            Err(_) => return text.to_string(),
        };
        let tag_map: HashMap<&str, &str> = tags
            .iter()
            .map(|t| (t.tag_name.as_str(), t.content.as_str()))
            .collect();

        tag_pattern
            .replace_all(text, |caps: &regex::Captures| {
                let tag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                match tag_map.get(tag_name) {
                    Some(content) => (*content).to_string(),
                    None => caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string(),
                }
            })
            .into_owned()
    }

    async fn resolve_session_id(
        &self,
        session_id: Option<Uuid>,
        attempt_id: Option<Uuid>,
        retry_tool: &'static str,
    ) -> Result<Uuid, CallToolResult> {
        match (session_id, attempt_id) {
            (Some(session_id), None) => Ok(session_id),
            (None, Some(attempt_id)) => {
                let session = Session::find_latest_by_workspace_id(&self.deployment.db().pool, attempt_id)
                    .await
                    .map_err(|e| {
                        Self::err_with(
                            "Failed to resolve latest session for attempt",
                            Some(json!({ "error": e.to_string(), "attempt_id": attempt_id })),
                            None,
                            Some("internal_error"),
                            Some(false),
                        )
                        .unwrap()
                    })?;

                if let Some(session) = session {
                    return Ok(session.id);
                }

                Err(Self::err_with(
                    "No session exists for this attempt yet.",
                    Some(json!({ "attempt_id": attempt_id.to_string() })),
                    Some(format!(
                        "Call tail_attempt_feed(attempt_id) and retry {retry_tool} once latest_session_id is non-null."
                    )),
                    Some(MCP_CODE_NO_SESSION_YET),
                    Some(true),
                )
                .unwrap())
            }
            (Some(session_id), Some(attempt_id)) => Err(Self::err_with(
                "Provide exactly one target identifier (attempt_id OR session_id).",
                Some(json!({
                    "attempt_id": attempt_id.to_string(),
                    "session_id": session_id.to_string(),
                })),
                Some(
                    "Remove one of {attempt_id, session_id}. If you only have a task_id, call list_task_attempts first."
                        .to_string(),
                ),
                Some(MCP_CODE_AMBIGUOUS_TARGET),
                Some(false),
            )
            .unwrap()),
            (None, None) => Err(Self::err_with(
                "Missing target identifier (attempt_id OR session_id is required).",
                None,
                Some(
                    "Provide attempt_id from list_task_attempts, or session_id from tail_attempt_feed."
                        .to_string(),
                ),
                Some(MCP_CODE_AMBIGUOUS_TARGET),
                Some(false),
            )
            .unwrap()),
        }
    }

    fn default_peer_client_id(&self) -> Option<String> {
        let guard = self.peer.read().ok()?;
        let peer = guard.as_ref()?;
        peer.peer_info()
            .map(|info| format!("mcp:{}@{}", info.client_info.name, info.client_info.version))
    }

    fn normalize_claimed_by_client_id(&self, raw: Option<String>) -> String {
        let provided = raw.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        provided
            .or_else(|| self.default_peer_client_id())
            .unwrap_or_else(|| "mcp:unknown".to_string())
    }

    #[allow(clippy::result_large_err)]
    fn lease_ttl(ttl_secs: Option<i64>) -> Result<chrono::Duration, CallToolResult> {
        let ttl_secs = ttl_secs.unwrap_or(DEFAULT_ATTEMPT_CONTROL_LEASE_TTL_SECS);
        if ttl_secs <= 0 {
            return Err(Self::err_with(
                "ttl_secs must be positive.",
                Some(json!({ "ttl_secs": ttl_secs })),
                Some("Provide ttl_secs > 0.".to_string()),
                Some("invalid_argument"),
                Some(false),
            )
            .unwrap());
        }

        if ttl_secs > ATTEMPT_CONTROL_LEASE_MAX_TTL_SECS {
            return Err(Self::err_with(
                "ttl_secs exceeds the server limit.",
                Some(json!({
                    "ttl_secs": ttl_secs,
                    "max_ttl_secs": ATTEMPT_CONTROL_LEASE_MAX_TTL_SECS,
                })),
                Some(format!(
                    "Reduce ttl_secs to <= {}.",
                    ATTEMPT_CONTROL_LEASE_MAX_TTL_SECS
                )),
                Some("invalid_argument"),
                Some(false),
            )
            .unwrap());
        }

        Ok(chrono::Duration::seconds(ttl_secs))
    }

    async fn require_attempt_control_token(
        &self,
        attempt_id: Uuid,
        control_token: Option<Uuid>,
        operation: &'static str,
    ) -> Result<(), CallToolResult> {
        let pool = &self.deployment.db().pool;
        let now = chrono::Utc::now();

        let lease = attempt_control_lease_model::get_by_attempt_id(pool, attempt_id)
            .await
            .map_err(|e| {
                Self::err_with(
                    "Failed to load attempt control lease",
                    Some(json!({
                        "error": e.to_string(),
                        "attempt_id": attempt_id,
                        "operation": operation,
                    })),
                    None,
                    Some("internal_error"),
                    Some(false),
                )
                .unwrap()
            })?;

        let Some(lease) = lease else {
            return Err(Self::err_with(
                "Attempt control lease is required for this operation.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "operation": operation,
                })),
                Some(
                    "Call claim_attempt_control(attempt_id) to obtain a control_token.".to_string(),
                ),
                Some(MCP_CODE_ATTEMPT_CLAIM_REQUIRED),
                Some(false),
            )
            .unwrap());
        };

        let expired = lease.is_expired_at(now);
        if expired {
            return Err(Self::err_with(
                "Attempt control lease has expired.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "operation": operation,
                    "claimed_by_client_id": lease.claimed_by_client_id,
                    "expires_at": lease.expires_at.to_rfc3339(),
                })),
                Some("Call claim_attempt_control(attempt_id) to renew the lease.".to_string()),
                Some(match control_token {
                    Some(_) => MCP_CODE_INVALID_CONTROL_TOKEN,
                    None => MCP_CODE_ATTEMPT_CLAIM_REQUIRED,
                }),
                Some(false),
            )
            .unwrap());
        }

        match control_token {
            None => Err(Self::err_with(
                "Missing control_token for mutating attempt operation.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "operation": operation,
                    "claimed_by_client_id": lease.claimed_by_client_id,
                    "expires_at": lease.expires_at.to_rfc3339(),
                })),
                Some("Provide the current control_token, or use claim_attempt_control(force=true) to take over.".to_string()),
                Some(MCP_CODE_ATTEMPT_CLAIM_CONFLICT),
                Some(false),
            )
            .unwrap()),
            Some(token) if token == lease.control_token => Ok(()),
            Some(token) => Err(Self::err_with(
                "Invalid control_token for attempt operation.",
                Some(json!({
                    "attempt_id": attempt_id,
                    "operation": operation,
                    "provided_control_token": token,
                    "claimed_by_client_id": lease.claimed_by_client_id,
                    "expires_at": lease.expires_at.to_rfc3339(),
                })),
                Some("Call claim_attempt_control(attempt_id) to obtain a fresh control_token.".to_string()),
                Some(MCP_CODE_INVALID_CONTROL_TOKEN),
                Some(false),
            )
            .unwrap()),
        }
    }

    fn map_attempt_state(
        status: Option<ExecutionProcessStatus>,
    ) -> (McpAttemptState, Option<String>) {
        match status {
            None => (McpAttemptState::Idle, None),
            Some(ExecutionProcessStatus::Running) => (McpAttemptState::Running, None),
            Some(ExecutionProcessStatus::Completed) => (McpAttemptState::Completed, None),
            Some(ExecutionProcessStatus::Failed) => {
                (McpAttemptState::Failed, Some("failed".to_string()))
            }
            Some(ExecutionProcessStatus::Killed) => {
                (McpAttemptState::Failed, Some("killed".to_string()))
            }
        }
    }

    async fn task_attempt_summaries(
        &self,
        task_ids: Vec<Uuid>,
    ) -> Result<HashMap<Uuid, TaskAttemptSummary>, DbErr> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let workspaces =
            Workspace::fetch_all_by_task_ids(&self.deployment.db().pool, &task_ids).await?;
        let workspace_ids: Vec<Uuid> = workspaces.iter().map(|w| w.id).collect();
        let sessions_by_workspace =
            Session::find_latest_by_workspace_ids(&self.deployment.db().pool, &workspace_ids)
                .await?;

        let mut latest_by_task: HashMap<Uuid, Workspace> = HashMap::new();
        for workspace in workspaces {
            match latest_by_task.entry(workspace.task_id) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(workspace);
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let current = entry.get();
                    let created_cmp = workspace.created_at.cmp(&current.created_at);
                    let is_newer = if created_cmp == std::cmp::Ordering::Equal {
                        workspace.id < current.id
                    } else {
                        created_cmp == std::cmp::Ordering::Greater
                    };
                    if is_newer {
                        entry.insert(workspace);
                    }
                }
            }
        }

        let mut summaries = HashMap::new();
        for task_id in task_ids {
            if let Some(workspace) = latest_by_task.get(&task_id) {
                let session = sessions_by_workspace.get(&workspace.id);
                summaries.insert(
                    task_id,
                    TaskAttemptSummary {
                        latest_attempt_id: Some(workspace.id.to_string()),
                        latest_workspace_branch: Some(workspace.branch.clone()),
                        latest_session_id: session.map(|s| s.id.to_string()),
                        latest_session_executor: session.and_then(|s| s.executor.clone()),
                    },
                );
            } else {
                summaries.insert(task_id, TaskAttemptSummary::default());
            }
        }

        Ok(summaries)
    }

    fn approval_to_summary(approval: approval_model::Approval) -> McpApprovalSummary {
        McpApprovalSummary {
            approval_id: approval.id.clone(),
            attempt_id: approval.attempt_id.to_string(),
            execution_process_id: approval.execution_process_id.to_string(),
            tool_name: approval.tool_name,
            tool_call_id: approval.tool_call_id,
            tool_input: approval.tool_input,
            status: match approval.status {
                utils_core::approvals::ApprovalStatus::Pending => "pending".to_string(),
                utils_core::approvals::ApprovalStatus::Approved => "approved".to_string(),
                utils_core::approvals::ApprovalStatus::Denied { .. } => "denied".to_string(),
                utils_core::approvals::ApprovalStatus::TimedOut => "timed_out".to_string(),
            },
            created_at: approval.created_at.to_rfc3339(),
            timeout_at: approval.timeout_at.to_rfc3339(),
        }
    }

    fn activity_event_from_outbox(entry: EventOutboxEntry) -> ActivityEvent {
        ActivityEvent {
            event_id: entry.id,
            event_uuid: entry.uuid.to_string(),
            event_type: entry.event_type,
            entity_type: entry.entity_type,
            entity_uuid: entry.entity_uuid.to_string(),
            created_at: entry.created_at.to_rfc3339(),
            published_at: entry.published_at.map(|t| t.to_rfc3339()),
            payload: entry.payload,
        }
    }

    async fn project_id_for_event(
        &self,
        entry: &EventOutboxEntry,
        task_project_cache: &mut HashMap<Uuid, Uuid>,
        session_project_cache: &mut HashMap<Uuid, Uuid>,
    ) -> Option<Uuid> {
        let pool = &self.deployment.db().pool;
        match entry.event_type.as_str() {
            EVENT_PROJECT_CREATED | EVENT_PROJECT_UPDATED | EVENT_PROJECT_DELETED => {
                serde_json::from_value::<ProjectEventPayload>(entry.payload.clone())
                    .ok()
                    .map(|p| p.project_id)
            }
            EVENT_TASK_CREATED | EVENT_TASK_UPDATED | EVENT_TASK_DELETED => {
                serde_json::from_value::<TaskEventPayload>(entry.payload.clone())
                    .ok()
                    .map(|p| p.project_id)
            }
            EVENT_TASK_ORCHESTRATION_TRANSITION => serde_json::from_value::<
                TaskOrchestrationTransitionEventPayload,
            >(entry.payload.clone())
            .ok()
            .map(|p| p.project_id),
            EVENT_WORKSPACE_CREATED | EVENT_WORKSPACE_UPDATED | EVENT_WORKSPACE_DELETED => {
                let payload: WorkspaceEventPayload =
                    serde_json::from_value(entry.payload.clone()).ok()?;
                if let Some(project_id) = task_project_cache.get(&payload.task_id) {
                    return Some(*project_id);
                }
                let task = Task::find_by_id(pool, payload.task_id).await.ok()??;
                task_project_cache.insert(payload.task_id, task.project_id);
                Some(task.project_id)
            }
            EVENT_EXECUTION_PROCESS_CREATED
            | EVENT_EXECUTION_PROCESS_UPDATED
            | EVENT_EXECUTION_PROCESS_DELETED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone()).ok()?;
                if let Some(project_id) = session_project_cache.get(&payload.session_id) {
                    return Some(*project_id);
                }
                let session = Session::find_by_id(pool, payload.session_id).await.ok()??;
                let workspace = Workspace::find_by_id(pool, session.workspace_id)
                    .await
                    .ok()??;
                let task = Task::find_by_id(pool, workspace.task_id).await.ok()??;
                session_project_cache.insert(payload.session_id, task.project_id);
                Some(task.project_id)
            }
            _ => None,
        }
    }

    async fn task_id_for_event(
        &self,
        entry: &EventOutboxEntry,
        session_task_cache: &mut HashMap<Uuid, Uuid>,
    ) -> Option<Uuid> {
        let pool = &self.deployment.db().pool;
        match entry.event_type.as_str() {
            EVENT_TASK_CREATED | EVENT_TASK_UPDATED | EVENT_TASK_DELETED => {
                serde_json::from_value::<TaskEventPayload>(entry.payload.clone())
                    .ok()
                    .map(|p| p.task_id)
            }
            EVENT_TASK_ORCHESTRATION_TRANSITION => serde_json::from_value::<
                TaskOrchestrationTransitionEventPayload,
            >(entry.payload.clone())
            .ok()
            .map(|p| p.task_id),
            EVENT_WORKSPACE_CREATED | EVENT_WORKSPACE_UPDATED | EVENT_WORKSPACE_DELETED => {
                serde_json::from_value::<WorkspaceEventPayload>(entry.payload.clone())
                    .ok()
                    .map(|p| p.task_id)
            }
            EVENT_EXECUTION_PROCESS_CREATED
            | EVENT_EXECUTION_PROCESS_UPDATED
            | EVENT_EXECUTION_PROCESS_DELETED => {
                let payload: ExecutionProcessEventPayload =
                    serde_json::from_value(entry.payload.clone()).ok()?;
                if let Some(task_id) = session_task_cache.get(&payload.session_id) {
                    return Some(*task_id);
                }
                let session = Session::find_by_id(pool, payload.session_id).await.ok()??;
                let workspace = Workspace::find_by_id(pool, session.workspace_id)
                    .await
                    .ok()??;
                session_task_cache.insert(payload.session_id, workspace.task_id);
                Some(workspace.task_id)
            }
            _ => None,
        }
    }
}

#[tool_handler]
impl ServerHandler for TaskServer {
    #[allow(clippy::manual_async_fn)]
    fn enqueue_task(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CreateTaskResult, rmcp::ErrorData>> + Send + '_
    {
        async move {
            let pool = &self.deployment.db().pool;
            let _ = mcp_tool_task_model::delete_expired(pool).await;

            let created_by_client_id = context
                .peer
                .peer_info()
                .map(|info| format!("mcp:{}@{}", info.client_info.name, info.client_info.version));

            let tool_name = request.name.to_string();
            let tool_arguments =
                serde_json::Value::Object(request.arguments.clone().unwrap_or_default());

            let resumable = self
                .get_tool(&request.name)
                .and_then(|tool| tool.annotations)
                .and_then(|anno| anno.read_only_hint)
                .unwrap_or(false);

            let attempt_id = request
                .arguments
                .as_ref()
                .and_then(|args| args.get("attempt_id"))
                .and_then(|value| value.as_str())
                .and_then(|raw| Uuid::parse_str(raw).ok());

            let mut kanban_task_id = None;
            let mut project_id = None;
            if let Some(attempt_id) = attempt_id
                && let Ok(Some(workspace)) = Workspace::find_by_id(pool, attempt_id).await
            {
                kanban_task_id = Some(workspace.task_id);
                if let Ok(Some(task)) = Task::find_by_id(pool, workspace.task_id).await {
                    project_id = Some(task.project_id);
                }
            }

            let ttl_ms = self
                .mcp_tasks
                .ttl_ms
                .and_then(|value| i64::try_from(value).ok());
            let poll_interval_ms = i64::try_from(self.mcp_tasks.poll_interval_ms).ok();

            let task_id = Uuid::new_v4().to_string();
            let record = mcp_tool_task_model::insert_working(
                pool,
                task_id.clone(),
                created_by_client_id,
                tool_name.clone(),
                tool_arguments.clone(),
                attempt_id,
                kanban_task_id,
                project_id,
                ttl_ms,
                poll_interval_ms,
                resumable,
            )
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to persist MCP task",
                    Some(json!({ "error": e.to_string(), "tool": tool_name })),
                )
            })?;

            self.spawn_mcp_tool_task_execution(
                task_id.clone(),
                tool_name,
                tool_arguments,
                context.peer,
            )
            .await;

            Ok(CreateTaskResult {
                task: Self::mcp_protocol_task_from_record(&record),
            })
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn list_tasks(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListTasksResult, rmcp::ErrorData>>
    + Send
    + '_ {
        async move {
            let pool = &self.deployment.db().pool;
            let _ = mcp_tool_task_model::delete_expired(pool).await;

            let mut status = None;
            let mut attempt_id = None;
            let mut kanban_task_id = None;
            let mut project_id = None;
            let mut limit = 20_u64;

            if let Some(params) = request.as_ref()
                && let Some(meta) = params.meta.as_ref()
                && let Some(vk) = meta.get("vk").and_then(|v| v.as_object())
            {
                status = vk
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                attempt_id = vk
                    .get("attempt_id")
                    .and_then(|v| v.as_str())
                    .and_then(|raw| Uuid::parse_str(raw).ok());
                project_id = vk
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .and_then(|raw| Uuid::parse_str(raw).ok());
                kanban_task_id = vk
                    .get("kanban_task_id")
                    .or_else(|| vk.get("task_id"))
                    .and_then(|v| v.as_str())
                    .and_then(|raw| Uuid::parse_str(raw).ok());
                limit = vk
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(limit)
                    .clamp(1, 200);
            }

            let cursor = request
                .as_ref()
                .and_then(|params| params.cursor.as_ref())
                .map(|raw| raw.trim().to_string())
                .filter(|raw| !raw.is_empty());

            let cursor = match cursor {
                None => None,
                Some(raw) => Some(raw.parse::<i64>().map_err(|_| {
                    let mut details = serde_json::Map::new();
                    details.insert("tool".to_string(), json!("list_tasks"));
                    details.insert("path".to_string(), json!("cursor"));
                    details.insert("value".to_string(), json!(raw));
                    details.insert(
                        "expected".to_string(),
                        json!("A signed 64-bit integer cursor"),
                    );
                    details.insert("next_tools".to_string(), json!([]));
                    details.insert("example_args".to_string(), json!({ "cursor": 0 }));

                    ErrorData::invalid_params(
                        "Invalid cursor",
                        Some(crate::mcp::params::invalid_params_payload(
                            "invalid_argument",
                            "Provide a valid cursor (signed 64-bit integer) or omit it."
                                .to_string(),
                            details,
                        )),
                    )
                })?),
            };

            let (tasks, next_cursor) = mcp_tool_task_model::list(
                pool,
                status.as_deref(),
                attempt_id,
                kanban_task_id,
                project_id,
                limit,
                cursor,
            )
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to list MCP tasks",
                    Some(json!({ "error": e.to_string() })),
                )
            })?;

            Ok(rmcp::model::ListTasksResult {
                tasks: tasks
                    .into_iter()
                    .map(|task| Self::mcp_protocol_task_from_record(&task))
                    .collect(),
                next_cursor: next_cursor.map(|value| value.to_string()),
                total: None,
            })
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn get_task_info(
        &self,
        request: GetTaskInfoParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetTaskResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            let pool = &self.deployment.db().pool;
            let _ = mcp_tool_task_model::delete_expired(pool).await;

            let task = mcp_tool_task_model::find_by_task_id(pool, &request.task_id)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to load MCP task",
                        Some(json!({ "error": e.to_string(), "task_id": request.task_id })),
                    )
                })?
                .ok_or_else(|| {
                    ErrorData::resource_not_found(
                        "Task not found",
                        Some(json!({ "task_id": request.task_id })),
                    )
                })?;

            Ok(GetTaskResult {
                meta: request.meta,
                task: Self::mcp_protocol_task_from_record(&task),
            })
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn get_task_result(
        &self,
        request: GetTaskResultParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<GetTaskPayloadResult, rmcp::ErrorData>> + Send + '_
    {
        async move {
            let pool = &self.deployment.db().pool;
            let _ = mcp_tool_task_model::delete_expired(pool).await;

            let task = mcp_tool_task_model::find_by_task_id(pool, &request.task_id)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to load MCP task result",
                        Some(json!({ "error": e.to_string(), "task_id": request.task_id })),
                    )
                })?
                .ok_or_else(|| {
                    ErrorData::resource_not_found(
                        "Task not found",
                        Some(json!({ "task_id": request.task_id })),
                    )
                })?;

            if let Some(payload) = task.result_json {
                return Ok(GetTaskPayloadResult(payload));
            }
            if let Some(payload) = task.error_json {
                return Ok(GetTaskPayloadResult(payload));
            }

            Err(ErrorData::invalid_request(
                "Task result not available",
                Some(json!({ "task_id": request.task_id, "status": task.status })),
            ))
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CancelTaskResult, rmcp::ErrorData>> + Send + '_
    {
        async move {
            let pool = &self.deployment.db().pool;
            let _ = mcp_tool_task_model::delete_expired(pool).await;

            let current = mcp_tool_task_model::find_by_task_id(pool, &request.task_id)
                .await
                .map_err(|e| {
                    ErrorData::internal_error(
                        "Failed to load MCP task",
                        Some(json!({ "error": e.to_string(), "task_id": request.task_id })),
                    )
                })?
                .ok_or_else(|| {
                    ErrorData::resource_not_found(
                        "Task not found",
                        Some(json!({ "task_id": request.task_id })),
                    )
                })?;

            if current.status != "working" && current.status != "input_required" {
                return Ok(CancelTaskResult {
                    meta: request.meta,
                    task: Self::mcp_protocol_task_from_record(&current),
                });
            }

            let updated = mcp_tool_task_model::update_status(
                pool,
                &request.task_id,
                "cancelled",
                Some("Cancelled by client.".to_string()),
            )
            .await
            .map_err(|e| {
                ErrorData::internal_error(
                    "Failed to cancel task",
                    Some(json!({ "error": e.to_string(), "task_id": request.task_id })),
                )
            })?;

            {
                let mut running = self.mcp_tasks.running.lock().await;
                if let Some(running) = running.remove(&request.task_id) {
                    running.ct.cancel();
                    running.handle.abort();
                }
            }

            Ok(CancelTaskResult {
                meta: request.meta,
                task: Self::mcp_protocol_task_from_record(&updated),
            })
        }
    }

    fn initialize(
        &self,
        request: InitializeRequestParams,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> impl std::future::Future<Output = Result<rmcp::model::InitializeResult, rmcp::ErrorData>>
    + Send
    + '_ {
        // Default rmcp behavior: record peer info from initialize params once.
        if context.peer.peer_info().is_none() {
            context.peer.set_peer_info(request);
        }

        let peer = context.peer.clone();
        self.record_peer(peer.clone());
        self.start_approvals_elicitation_if_supported(peer.clone());
        self.spawn_mcp_task_resumer(peer);

        std::future::ready(Ok(self.get_info()))
    }

    fn get_info(&self) -> ServerInfo {
        let instruction = "Vibe Kanban MCP control plane (native mode). Recommended closed-loop: start_attempt (with optional prompt) → tail_attempt_feed (poll with after_log_index) → respond_approval (when pending approvals appear) → get_attempt_changes/get_attempt_patch/get_attempt_file as needed → stop_attempt. For broader observability, use tail_project_activity/tail_task_activity. Errors: invalid/ill-typed tool inputs return JSON-RPC invalid_params; business failures return tool-level structured errors in structuredContent with {code,retryable,hint,details}. Some clients may also support approvals via MCP elicitation (server push) when declared during initialize.".to_string();

        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks_with(TasksCapability::server_default())
                .build(),
            server_info: Implementation {
                name: "vibe-kanban".to_string(),
                title: Some("Vibe Kanban MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some(
                    "Local-first Kanban MCP control plane for multi-agent orchestration."
                        .to_string(),
                ),
                icons: Some(vec![Icon {
                    src: "https://raw.githubusercontent.com/StrayDragon/vibe-kanban/main/frontend/public/vibe-kanban-logo.svg".to_string(),
                    mime_type: Some("image/svg+xml".to_string()),
                    sizes: Some(vec!["any".to_string()]),
                }]),
                website_url: Some("https://www.vibekanban.com".to_string()),
            },
            instructions: Some(instruction),
        }
    }
}

// `axum::response::Json` wrapper type used by route handlers.
use axum::response::Json as ResponseJson;
