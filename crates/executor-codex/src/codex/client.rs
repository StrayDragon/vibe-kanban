use std::{
    collections::{HashMap, VecDeque},
    io,
    sync::{Arc, OnceLock},
};

use async_trait::async_trait;
use codex_app_server_protocol::{
    ApplyPatchApprovalResponse, ClientInfo, ClientNotification, ClientRequest,
    CommandExecutionApprovalDecision, CommandExecutionRequestApprovalResponse,
    DynamicToolCallOutputContentItem, DynamicToolCallResponse, ExecCommandApprovalResponse,
    ExecPolicyAmendment, FileChangeApprovalDecision, FileChangeRequestApprovalResponse,
    GetAuthStatusParams, GetAuthStatusResponse, GrantedPermissionProfile, InitializeCapabilities,
    InitializeParams, JSONRPCError, JSONRPCErrorError, JSONRPCNotification, JSONRPCRequest,
    JSONRPCResponse, McpServerElicitationAction, McpServerElicitationRequestResponse,
    PermissionGrantScope, PermissionsRequestApprovalResponse, RequestId, ServerRequest,
    ThreadForkParams, ThreadStartParams, ToolRequestUserInputAnswer, ToolRequestUserInputResponse,
    TurnStartParams, TurnStartResponse, UserInput,
};
use codex_protocol::protocol::{EventMsg, ReviewDecision};
use executors_core::{
    approvals::{ExecutorApprovalError, ExecutorApprovalService},
    executors::ExecutorError,
    log_writer::LogWriter,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{self, Value};
use tokio::sync::Mutex;
use utils_core::approvals::ApprovalStatus;

use super::{
    dynamic_tools::{VkDynamicToolContext, VkDynamicToolKind, VkDynamicToolRegistry},
    jsonrpc::{JsonRpcCallbacks, JsonRpcPeer},
    normalize_logs::Approval,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadResponseLite {
    thread: ThreadLite,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadLite {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexEventNotificationParams {
    msg: EventMsg,
}

pub struct AppServerClient {
    rpc: OnceLock<JsonRpcPeer>,
    log_writer: LogWriter,
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
    thread_id: Mutex<Option<String>>,
    pending_feedback: Mutex<VecDeque<String>>,
    auto_approve: bool,
    dynamic_tool_registry: VkDynamicToolRegistry,
    dynamic_tool_context: VkDynamicToolContext,
    recent_log_lines: Mutex<VecDeque<String>>,
}

impl AppServerClient {
    pub fn new(
        log_writer: LogWriter,
        approvals: Option<Arc<dyn ExecutorApprovalService>>,
        auto_approve: bool,
        dynamic_tool_context: VkDynamicToolContext,
    ) -> Arc<Self> {
        Arc::new(Self {
            rpc: OnceLock::new(),
            log_writer,
            approvals,
            auto_approve,
            dynamic_tool_registry: VkDynamicToolRegistry::vk_default(),
            dynamic_tool_context,
            thread_id: Mutex::new(None),
            pending_feedback: Mutex::new(VecDeque::new()),
            recent_log_lines: Mutex::new(VecDeque::new()),
        })
    }

    pub fn connect(&self, peer: JsonRpcPeer) {
        let _ = self.rpc.set(peer);
    }

    fn rpc(&self) -> &JsonRpcPeer {
        self.rpc.get().expect("Codex RPC peer not attached")
    }

    pub async fn initialize(&self) -> Result<(), ExecutorError> {
        let request = ClientRequest::Initialize {
            request_id: self.next_request_id(),
            params: InitializeParams {
                client_info: ClientInfo {
                    name: "vibe-codex-executor".to_string(),
                    title: None,
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: Some(InitializeCapabilities {
                    // VK uses experimental fields like `thread/start.dynamicTools`.
                    experimental_api: true,
                    opt_out_notification_methods: None,
                }),
            },
        };

        // The app-server initialize response evolves over time; we only need an acknowledgment.
        self.send_request::<Value>(request, "initialize").await?;
        self.send_message(&ClientNotification::Initialized).await
    }

    pub async fn thread_start(&self, params: ThreadStartParams) -> Result<String, ExecutorError> {
        let request = ClientRequest::ThreadStart {
            request_id: self.next_request_id(),
            params,
        };
        let response: ThreadResponseLite = self.send_request(request, "thread/start").await?;
        Ok(response.thread.id)
    }

    pub async fn thread_fork(&self, params: ThreadForkParams) -> Result<String, ExecutorError> {
        let request = ClientRequest::ThreadFork {
            request_id: self.next_request_id(),
            params,
        };
        let response: ThreadResponseLite = self.send_request(request, "thread/fork").await?;
        Ok(response.thread.id)
    }

    pub async fn turn_start(
        &self,
        thread_id: &str,
        input: Vec<UserInput>,
    ) -> Result<TurnStartResponse, ExecutorError> {
        let request = ClientRequest::TurnStart {
            request_id: self.next_request_id(),
            params: TurnStartParams {
                thread_id: thread_id.to_string(),
                input,
                cwd: None,
                approval_policy: None,
                sandbox_policy: None,
                model: None,
                service_tier: None,
                effort: None,
                summary: None,
                personality: None,
                output_schema: None,
                collaboration_mode: None,
            },
        };
        self.send_request(request, "turn/start").await
    }

    pub async fn get_auth_status(&self) -> Result<GetAuthStatusResponse, ExecutorError> {
        let request = ClientRequest::GetAuthStatus {
            request_id: self.next_request_id(),
            params: GetAuthStatusParams {
                include_token: Some(true),
                refresh_token: Some(false),
            },
        };
        self.send_request(request, "getAuthStatus").await
    }
    async fn handle_server_request(
        &self,
        peer: &JsonRpcPeer,
        request: ServerRequest,
    ) -> Result<(), ExecutorError> {
        match request {
            ServerRequest::ApplyPatchApproval { request_id, params } => {
                let input = serde_json::to_value(&params)
                    .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?;
                let status = match self
                    .request_tool_approval("edit", input, &params.call_id)
                    .await
                {
                    Ok(status) => status,
                    Err(err) => {
                        tracing::error!("failed to request patch approval: {err}");
                        ApprovalStatus::Denied {
                            reason: Some("approval service error".to_string()),
                        }
                    }
                };
                self.log_writer
                    .log_raw(
                        &Approval::approval_response(
                            params.call_id,
                            "codex.apply_patch".to_string(),
                            status.clone(),
                        )
                        .raw(),
                    )
                    .await?;
                let (decision, feedback) = self.review_decision(&status).await?;
                let response = ApplyPatchApprovalResponse { decision };
                send_server_response(peer, request_id, response).await?;
                if let Some(message) = feedback {
                    tracing::debug!("queueing patch denial feedback: {message}");
                    self.enqueue_feedback(message).await;
                }
                Ok(())
            }
            ServerRequest::ExecCommandApproval { request_id, params } => {
                let input = serde_json::to_value(&params)
                    .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?;
                let status = match self
                    .request_tool_approval("bash", input, &params.call_id)
                    .await
                {
                    Ok(status) => status,
                    Err(err) => {
                        tracing::error!("failed to request command approval: {err}");
                        ApprovalStatus::Denied {
                            reason: Some("approval service error".to_string()),
                        }
                    }
                };
                self.log_writer
                    .log_raw(
                        &Approval::approval_response(
                            params.call_id,
                            "codex.exec_command".to_string(),
                            status.clone(),
                        )
                        .raw(),
                    )
                    .await?;

                let (decision, feedback) = self.review_decision(&status).await?;
                let response = ExecCommandApprovalResponse { decision };
                send_server_response(peer, request_id, response).await?;
                if let Some(message) = feedback {
                    tracing::debug!("queueing exec denial feedback: {message}");
                    self.enqueue_feedback(message).await;
                }
                Ok(())
            }
            ServerRequest::CommandExecutionRequestApproval { request_id, params } => {
                let input = serde_json::to_value(&params)
                    .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?;

                let call_id = params.item_id.clone();

                let status = match self.request_tool_approval("bash", input, &call_id).await {
                    Ok(status) => status,
                    Err(err) => {
                        tracing::error!("failed to request command approval: {err}");
                        ApprovalStatus::Denied {
                            reason: Some("approval service error".to_string()),
                        }
                    }
                };

                self.log_writer
                    .log_raw(
                        &Approval::approval_response(
                            call_id.clone(),
                            "codex.exec_command".to_string(),
                            status.clone(),
                        )
                        .raw(),
                    )
                    .await?;

                let (review_decision, feedback) = self.review_decision(&status).await?;
                let decision = match review_decision {
                    ReviewDecision::Approved => CommandExecutionApprovalDecision::Accept,
                    ReviewDecision::ApprovedForSession => {
                        CommandExecutionApprovalDecision::AcceptForSession
                    }
                    ReviewDecision::ApprovedExecpolicyAmendment {
                        proposed_execpolicy_amendment,
                    } => CommandExecutionApprovalDecision::AcceptWithExecpolicyAmendment {
                        execpolicy_amendment: ExecPolicyAmendment::from(
                            proposed_execpolicy_amendment,
                        ),
                    },
                    ReviewDecision::NetworkPolicyAmendment {
                        network_policy_amendment,
                    } => CommandExecutionApprovalDecision::ApplyNetworkPolicyAmendment {
                        network_policy_amendment: network_policy_amendment.into(),
                    },
                    ReviewDecision::Denied => CommandExecutionApprovalDecision::Decline,
                    ReviewDecision::Abort => CommandExecutionApprovalDecision::Cancel,
                };
                let response = CommandExecutionRequestApprovalResponse { decision };
                send_server_response(peer, request_id, response).await?;
                if let Some(message) = feedback {
                    tracing::debug!("queueing exec denial feedback: {message}");
                    self.enqueue_feedback(message).await;
                }
                Ok(())
            }
            ServerRequest::FileChangeRequestApproval { request_id, params } => {
                let input = serde_json::to_value(&params)
                    .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?;

                let status = match self
                    .request_tool_approval("edit", input, &params.item_id)
                    .await
                {
                    Ok(status) => status,
                    Err(err) => {
                        tracing::error!("failed to request file-change approval: {err}");
                        ApprovalStatus::Denied {
                            reason: Some("approval service error".to_string()),
                        }
                    }
                };

                self.log_writer
                    .log_raw(
                        &Approval::approval_response(
                            params.item_id.clone(),
                            "codex.apply_patch".to_string(),
                            status.clone(),
                        )
                        .raw(),
                    )
                    .await?;

                let (review_decision, feedback) = self.review_decision(&status).await?;
                let decision = match review_decision {
                    ReviewDecision::Approved
                    | ReviewDecision::ApprovedExecpolicyAmendment { .. } => {
                        FileChangeApprovalDecision::Accept
                    }
                    ReviewDecision::ApprovedForSession => {
                        FileChangeApprovalDecision::AcceptForSession
                    }
                    ReviewDecision::NetworkPolicyAmendment { .. } => {
                        FileChangeApprovalDecision::Decline
                    }
                    ReviewDecision::Denied => FileChangeApprovalDecision::Decline,
                    ReviewDecision::Abort => FileChangeApprovalDecision::Cancel,
                };
                let response = FileChangeRequestApprovalResponse { decision };
                send_server_response(peer, request_id, response).await?;
                if let Some(message) = feedback {
                    tracing::debug!("queueing edit denial feedback: {message}");
                    self.enqueue_feedback(message).await;
                }
                Ok(())
            }
            ServerRequest::ToolRequestUserInput { request_id, params } => {
                tracing::warn!(
                    thread_id = %params.thread_id,
                    turn_id = %params.turn_id,
                    item_id = %params.item_id,
                    "ToolRequestUserInput is unsupported; responding with empty answers"
                );
                let answers = params
                    .questions
                    .into_iter()
                    .map(|question| {
                        (
                            question.id,
                            ToolRequestUserInputAnswer {
                                answers: Vec::new(),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>();
                let response = ToolRequestUserInputResponse { answers };
                send_server_response(peer, request_id, response).await
            }
            ServerRequest::McpServerElicitationRequest { request_id, params } => {
                tracing::warn!(
                    thread_id = %params.thread_id,
                    server_name = %params.server_name,
                    "McpServerElicitationRequest is unsupported; declining"
                );
                let response = McpServerElicitationRequestResponse {
                    action: McpServerElicitationAction::Decline,
                    content: None,
                    meta: None,
                };
                send_server_response(peer, request_id, response).await
            }
            ServerRequest::PermissionsRequestApproval { request_id, params } => {
                tracing::warn!(
                    thread_id = %params.thread_id,
                    turn_id = %params.turn_id,
                    item_id = %params.item_id,
                    "PermissionsRequestApproval is unsupported; granting none"
                );
                let response = PermissionsRequestApprovalResponse {
                    permissions: GrantedPermissionProfile::default(),
                    scope: PermissionGrantScope::default(),
                };
                send_server_response(peer, request_id, response).await
            }
            ServerRequest::DynamicToolCall { request_id, params } => {
                let tool = params.tool.clone();
                if !self.dynamic_tool_registry.is_supported(&tool) {
                    let response = DynamicToolCallResponse {
                        content_items: vec![DynamicToolCallOutputContentItem::InputText {
                            text: format!("Unsupported dynamic tool: `{tool}`"),
                        }],
                        success: false,
                    };
                    return send_server_response(peer, request_id, response).await;
                }

                let kind = self
                    .dynamic_tool_registry
                    .kind(&tool)
                    .unwrap_or(VkDynamicToolKind::ReadOnly);

                if kind == VkDynamicToolKind::Mutating {
                    let status = match self
                        .request_tool_approval(&tool, params.arguments.clone(), &params.call_id)
                        .await
                    {
                        Ok(status) => status,
                        Err(err) => {
                            tracing::error!("failed to request dynamic tool approval: {err}");
                            ApprovalStatus::Denied {
                                reason: Some("approval service error".to_string()),
                            }
                        }
                    };

                    self.log_writer
                        .log_raw(
                            &Approval::approval_response(
                                params.call_id.clone(),
                                tool.clone(),
                                status.clone(),
                            )
                            .raw(),
                        )
                        .await?;

                    if !matches!(status, ApprovalStatus::Approved) {
                        let response = DynamicToolCallResponse {
                            content_items: vec![DynamicToolCallOutputContentItem::InputText {
                                text:
                                    "Dynamic tool execution requires approval and was not approved."
                                        .to_string(),
                            }],
                            success: false,
                        };
                        return send_server_response(peer, request_id, response).await;
                    }
                }

                let recent_logs = self.recent_log_snapshot().await;
                let result = self
                    .dynamic_tool_registry
                    .execute(
                        &self.dynamic_tool_context,
                        &tool,
                        params.arguments,
                        Some(recent_logs),
                    )
                    .await;

                let response = match result {
                    Ok(content_items) => DynamicToolCallResponse {
                        content_items,
                        success: true,
                    },
                    Err(message) => DynamicToolCallResponse {
                        content_items: vec![DynamicToolCallOutputContentItem::InputText {
                            text: message,
                        }],
                        success: false,
                    },
                };

                send_server_response(peer, request_id, response).await
            }
            ServerRequest::ChatgptAuthTokensRefresh {
                request_id,
                params: _,
            } => {
                tracing::error!("ChatgptAuthTokensRefresh is unsupported by this client");
                peer.send(&JSONRPCError {
                    id: request_id,
                    error: JSONRPCErrorError {
                        code: -32601,
                        message: "ChatgptAuthTokensRefresh is unsupported by this client"
                            .to_string(),
                        data: None,
                    },
                })
                .await
            }
        }
    }

    async fn request_tool_approval(
        &self,
        tool_name: &str,
        tool_input: Value,
        tool_call_id: &str,
    ) -> Result<ApprovalStatus, ExecutorError> {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        if self.auto_approve {
            return Ok(ApprovalStatus::Approved);
        }
        Ok(self
            .approvals
            .as_ref()
            .ok_or(ExecutorApprovalError::ServiceUnavailable)?
            .request_tool_approval(tool_name, tool_input, tool_call_id)
            .await?)
    }

    pub async fn register_thread(&self, thread_id: &str) -> Result<(), ExecutorError> {
        {
            let mut guard = self.thread_id.lock().await;
            guard.replace(thread_id.to_string());
        }
        self.flush_pending_feedback().await;
        Ok(())
    }

    async fn send_message<M>(&self, message: &M) -> Result<(), ExecutorError>
    where
        M: Serialize + Sync,
    {
        self.rpc().send(message).await
    }

    async fn send_request<R>(&self, request: ClientRequest, label: &str) -> Result<R, ExecutorError>
    where
        R: DeserializeOwned + std::fmt::Debug,
    {
        let request_id = request_id(&request);
        self.rpc().request(request_id, &request, label).await
    }

    fn next_request_id(&self) -> RequestId {
        self.rpc().next_request_id()
    }

    async fn record_recent_log_line(&self, raw: &str) {
        const MAX_RECENT_LOG_LINES: usize = 500;
        const MAX_LINE_BYTES: usize = 4000;

        let line = if raw.len() > MAX_LINE_BYTES {
            let mut truncated =
                utils_core::text::truncate_to_char_boundary(raw, MAX_LINE_BYTES).to_string();
            truncated.push_str("…(truncated)");
            truncated
        } else {
            raw.to_string()
        };

        let mut guard = self.recent_log_lines.lock().await;
        guard.push_back(line);
        while guard.len() > MAX_RECENT_LOG_LINES {
            guard.pop_front();
        }
    }

    async fn recent_log_snapshot(&self) -> Vec<String> {
        self.recent_log_lines
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    }

    async fn review_decision(
        &self,
        status: &ApprovalStatus,
    ) -> Result<(ReviewDecision, Option<String>), ExecutorError> {
        if self.auto_approve {
            return Ok((ReviewDecision::ApprovedForSession, None));
        }

        let outcome = match status {
            ApprovalStatus::Approved => (ReviewDecision::Approved, None),
            ApprovalStatus::Denied { reason } => {
                let feedback = reason
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                if feedback.is_some() {
                    (ReviewDecision::Abort, feedback)
                } else {
                    (ReviewDecision::Denied, None)
                }
            }
            ApprovalStatus::TimedOut => (ReviewDecision::Denied, None),
            ApprovalStatus::Pending => (ReviewDecision::Denied, None),
        };
        Ok(outcome)
    }

    async fn enqueue_feedback(&self, message: String) {
        if message.trim().is_empty() {
            return;
        }
        let mut guard = self.pending_feedback.lock().await;
        guard.push_back(message);
    }

    async fn flush_pending_feedback(&self) {
        let messages: Vec<String> = {
            let mut guard = self.pending_feedback.lock().await;
            guard.drain(..).collect()
        };

        if messages.is_empty() {
            return;
        }

        let Some(thread_id) = self.thread_id.lock().await.clone() else {
            tracing::warn!(
                "pending Codex feedback but thread id unavailable; dropping {} messages",
                messages.len()
            );
            return;
        };

        for message in messages {
            let trimmed = message.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.spawn_feedback_message(thread_id.clone(), trimmed.to_string());
        }
    }

    fn spawn_feedback_message(&self, thread_id: String, feedback: String) {
        let peer = self.rpc().clone();
        let request = ClientRequest::TurnStart {
            request_id: peer.next_request_id(),
            params: TurnStartParams {
                thread_id,
                input: vec![UserInput::Text {
                    text: format!("User feedback: {feedback}"),
                    text_elements: Vec::new(),
                }],
                cwd: None,
                approval_policy: None,
                sandbox_policy: None,
                model: None,
                service_tier: None,
                effort: None,
                summary: None,
                personality: None,
                output_schema: None,
                collaboration_mode: None,
            },
        };
        tokio::spawn(async move {
            if let Err(err) = peer
                .request::<TurnStartResponse, _>(request_id(&request), &request, "turn/start")
                .await
            {
                tracing::error!("failed to send feedback follow-up message: {err}");
            }
        });
    }
}

#[async_trait]
impl JsonRpcCallbacks for AppServerClient {
    async fn on_request(
        &self,
        peer: &JsonRpcPeer,
        raw: &str,
        request: JSONRPCRequest,
    ) -> Result<(), ExecutorError> {
        self.record_recent_log_line(raw).await;
        self.log_writer.log_raw(raw).await?;
        match ServerRequest::try_from(request.clone()) {
            Ok(server_request) => self.handle_server_request(peer, server_request).await,
            Err(err) => {
                tracing::debug!("Unhandled server request `{}`: {err}", request.method);
                let response = JSONRPCResponse {
                    id: request.id,
                    result: Value::Null,
                };
                peer.send(&response).await
            }
        }
    }

    async fn on_response(
        &self,
        _peer: &JsonRpcPeer,
        raw: &str,
        _response: &JSONRPCResponse,
    ) -> Result<(), ExecutorError> {
        self.record_recent_log_line(raw).await;
        self.log_writer.log_raw(raw).await
    }

    async fn on_error(
        &self,
        _peer: &JsonRpcPeer,
        raw: &str,
        _error: &JSONRPCError,
    ) -> Result<(), ExecutorError> {
        self.record_recent_log_line(raw).await;
        self.log_writer.log_raw(raw).await
    }

    async fn on_notification(
        &self,
        _peer: &JsonRpcPeer,
        raw: &str,
        notification: JSONRPCNotification,
    ) -> Result<bool, ExecutorError> {
        self.record_recent_log_line(raw).await;
        self.log_writer.log_raw(raw).await?;

        let method = notification.method.as_str();
        if !method.starts_with("codex/event") {
            return Ok(false);
        }

        if method.ends_with("turn_aborted") {
            tracing::debug!("codex turn aborted; flushing feedback queue");
            self.flush_pending_feedback().await;
            return Ok(false);
        }

        let has_finished = method
            .strip_prefix("codex/event/")
            .is_some_and(|suffix| matches!(suffix, "task_complete" | "turn_complete"))
            || notification
                .params
                .as_ref()
                .and_then(|params| {
                    serde_json::from_value::<CodexEventNotificationParams>(params.clone()).ok()
                })
                .is_some_and(|params| matches!(params.msg, EventMsg::TurnComplete(_)));

        Ok(has_finished)
    }

    async fn on_non_json(&self, raw: &str) -> Result<(), ExecutorError> {
        self.record_recent_log_line(raw).await;
        self.log_writer.log_raw(raw).await?;
        Ok(())
    }
}

async fn send_server_response<T>(
    peer: &JsonRpcPeer,
    request_id: RequestId,
    response: T,
) -> Result<(), ExecutorError>
where
    T: Serialize,
{
    let payload = JSONRPCResponse {
        id: request_id,
        result: serde_json::to_value(response)
            .map_err(|err| ExecutorError::Io(io::Error::other(err.to_string())))?,
    };

    peer.send(&payload).await
}

fn request_id(request: &ClientRequest) -> RequestId {
    match request {
        ClientRequest::Initialize { request_id, .. }
        | ClientRequest::GetAuthStatus { request_id, .. }
        | ClientRequest::ThreadStart { request_id, .. }
        | ClientRequest::ThreadFork { request_id, .. }
        | ClientRequest::TurnStart { request_id, .. } => request_id.clone(),
        _ => unreachable!("request_id called for unsupported request variant"),
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::{process::Stdio, sync::Arc};

    use async_trait::async_trait;
    use codex_app_server_protocol::DynamicToolCallParams;
    use tokio::{
        process::Command,
        sync::{Mutex, oneshot},
        time::{Duration, timeout},
    };

    use super::{super::jsonrpc::ExitSignalSender, *};

    #[derive(Default)]
    struct ResponseState {
        responses: Vec<JSONRPCResponse>,
    }

    #[derive(Clone)]
    struct RecordingCallbacks {
        state: Arc<Mutex<ResponseState>>,
    }

    #[async_trait]
    impl JsonRpcCallbacks for RecordingCallbacks {
        async fn on_request(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            _request: JSONRPCRequest,
        ) -> Result<(), ExecutorError> {
            Ok(())
        }

        async fn on_response(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            response: &JSONRPCResponse,
        ) -> Result<(), ExecutorError> {
            self.state.lock().await.responses.push(response.clone());
            Ok(())
        }

        async fn on_error(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            _error: &JSONRPCError,
        ) -> Result<(), ExecutorError> {
            Ok(())
        }

        async fn on_notification(
            &self,
            _peer: &JsonRpcPeer,
            _raw: &str,
            _notification: JSONRPCNotification,
        ) -> Result<bool, ExecutorError> {
            Ok(false)
        }

        async fn on_non_json(&self, _raw: &str) -> Result<(), ExecutorError> {
            Ok(())
        }
    }

    async fn spawn_peer(state: Arc<Mutex<ResponseState>>) -> JsonRpcPeer {
        let callbacks = Arc::new(RecordingCallbacks { state });
        let mut child = Command::new("sh")
            .arg("-c")
            .arg("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn cat");
        let stdout = child.stdout.take().expect("stdout");
        let stdin = child.stdin.take().expect("stdin");
        let (exit_tx, _exit_rx) = oneshot::channel();

        let peer = JsonRpcPeer::spawn(stdin, stdout, callbacks, ExitSignalSender::new(exit_tx));
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        peer
    }

    async fn wait_for_response(state: Arc<Mutex<ResponseState>>) -> JSONRPCResponse {
        timeout(Duration::from_secs(1), async move {
            loop {
                if let Some(response) = state.lock().await.responses.first().cloned() {
                    return response;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("response timeout")
    }

    #[tokio::test]
    async fn on_request_unknown_method_sends_null_result() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state.clone()).await;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            true,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let request = JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "unknown.method".to_string(),
            params: None,
            trace: None,
        };

        client
            .on_request(&peer, "raw", request)
            .await
            .expect("on_request");

        let response = wait_for_response(state).await;
        assert_eq!(response.id, RequestId::Integer(1));
        assert_eq!(response.result, Value::Null);
    }

    #[tokio::test]
    async fn record_recent_log_line_truncates_on_utf8_boundary() {
        const MAX_LINE_BYTES: usize = 4000;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            true,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let samples = vec![
            "你".repeat(4001),
            "あ".repeat(4001),
            format!("a{}", "é".repeat(5000)),
            format!("a{}", "🔥".repeat(2000)),
            format!("a{}", "مرحبا".repeat(2000)),
        ];

        for raw in &samples {
            client.record_recent_log_line(raw).await;
        }

        let snapshot = client.recent_log_snapshot().await;
        assert_eq!(snapshot.len(), samples.len());
        for (idx, raw) in samples.iter().enumerate() {
            assert!(snapshot[idx].ends_with("…(truncated)"));
            let expected_prefix = utils_core::text::truncate_to_char_boundary(raw, MAX_LINE_BYTES);
            assert!(snapshot[idx].starts_with(expected_prefix));
        }
    }

    #[tokio::test]
    async fn on_notification_turn_aborted_flushes_pending_feedback() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state).await;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            false,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        client.enqueue_feedback("feedback".to_string()).await;
        assert_eq!(client.pending_feedback.lock().await.len(), 1);

        let notification = JSONRPCNotification {
            method: "codex/event/turn_aborted".to_string(),
            params: None,
        };

        let finished = client
            .on_notification(&peer, "raw", notification)
            .await
            .expect("notification");

        assert!(!finished);
        assert!(client.pending_feedback.lock().await.is_empty());
    }

    #[tokio::test]
    async fn on_notification_task_complete_returns_true() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state).await;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            false,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let notification = JSONRPCNotification {
            method: "codex/event/task_complete".to_string(),
            params: None,
        };

        let finished = client
            .on_notification(&peer, "raw", notification)
            .await
            .expect("notification");

        assert!(finished);
    }

    #[tokio::test]
    async fn on_notification_turn_complete_returns_true() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state).await;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            false,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let notification = JSONRPCNotification {
            method: "codex/event/turn_complete".to_string(),
            params: None,
        };

        let finished = client
            .on_notification(&peer, "raw", notification)
            .await
            .expect("notification");

        assert!(finished);
    }

    #[tokio::test]
    async fn on_notification_turn_complete_in_params_returns_true() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state).await;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            false,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let notification = JSONRPCNotification {
            method: "codex/event".to_string(),
            params: Some(serde_json::json!({
                "msg": { "type": "turn_complete", "turn_id": "turn-1", "last_agent_message": null }
            })),
        };

        let finished = client
            .on_notification(&peer, "raw", notification)
            .await
            .expect("notification");

        assert!(finished);
    }

    #[tokio::test]
    async fn on_notification_non_codex_event_returns_false() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state).await;
        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            false,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let notification = JSONRPCNotification {
            method: "other/event".to_string(),
            params: None,
        };

        let finished = client
            .on_notification(&peer, "raw", notification)
            .await
            .expect("notification");

        assert!(!finished);
    }

    #[tokio::test]
    async fn dynamic_tool_call_unknown_tool_returns_failure() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state.clone()).await;

        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            None,
            true,
            VkDynamicToolContext::new(std::env::temp_dir()),
        );

        let request = ServerRequest::DynamicToolCall {
            request_id: RequestId::Integer(1),
            params: DynamicToolCallParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                call_id: "call-1".to_string(),
                tool: "vk.unknown".to_string(),
                arguments: serde_json::json!({}),
            },
        };

        client
            .handle_server_request(&peer, request)
            .await
            .expect("handle request");

        let response = wait_for_response(state).await;
        let parsed: DynamicToolCallResponse =
            serde_json::from_value(response.result).expect("decode response");
        assert!(!parsed.success);
        assert!(matches!(
            parsed.content_items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains("Unsupported dynamic tool") && text.contains("vk.unknown")
        ));
    }

    #[tokio::test]
    async fn dynamic_tool_call_invalid_args_returns_failure() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state.clone()).await;

        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some("3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b".to_string());
        let client = AppServerClient::new(LogWriter::new(tokio::io::sink()), None, true, ctx);

        let request = ServerRequest::DynamicToolCall {
            request_id: RequestId::Integer(1),
            params: DynamicToolCallParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                call_id: "call-1".to_string(),
                tool: crate::codex::dynamic_tools::VK_TOOL_GET_ATTEMPT_STATUS.to_string(),
                arguments: serde_json::json!({ "attempt_id": 123 }),
            },
        };

        client
            .handle_server_request(&peer, request)
            .await
            .expect("handle request");

        let response = wait_for_response(state).await;
        let parsed: DynamicToolCallResponse =
            serde_json::from_value(response.result).expect("decode response");
        assert!(!parsed.success);
        assert!(matches!(
            parsed.content_items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains("Invalid arguments")
                    && text.contains(crate::codex::dynamic_tools::VK_TOOL_GET_ATTEMPT_STATUS)
        ));
    }

    #[tokio::test]
    async fn dynamic_tool_call_read_only_tool_succeeds() {
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state.clone()).await;

        let attempt_id = "3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b";
        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some(attempt_id.to_string());
        let client = AppServerClient::new(LogWriter::new(tokio::io::sink()), None, true, ctx);

        let request = ServerRequest::DynamicToolCall {
            request_id: RequestId::Integer(1),
            params: DynamicToolCallParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                call_id: "call-1".to_string(),
                tool: crate::codex::dynamic_tools::VK_TOOL_GET_ATTEMPT_STATUS.to_string(),
                arguments: serde_json::json!({}),
            },
        };

        client
            .handle_server_request(&peer, request)
            .await
            .expect("handle request");

        let response = wait_for_response(state).await;
        let parsed: DynamicToolCallResponse =
            serde_json::from_value(response.result).expect("decode response");
        assert!(parsed.success);
        assert!(matches!(
            parsed.content_items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains(&format!("attempt_id: {attempt_id}"))
        ));
    }

    #[tokio::test]
    async fn dynamic_tool_call_mutating_tool_requests_approval_and_denies() {
        #[derive(Default)]
        struct RecordingApprovalService {
            calls: Mutex<Vec<(String, String)>>,
        }

        #[async_trait]
        impl ExecutorApprovalService for RecordingApprovalService {
            async fn request_tool_approval(
                &self,
                tool_name: &str,
                _tool_input: Value,
                tool_call_id: &str,
            ) -> Result<ApprovalStatus, ExecutorApprovalError> {
                self.calls
                    .lock()
                    .await
                    .push((tool_name.to_string(), tool_call_id.to_string()));
                Ok(ApprovalStatus::Denied {
                    reason: Some("no".to_string()),
                })
            }
        }

        let approvals = Arc::new(RecordingApprovalService::default());
        let state = Arc::new(Mutex::new(ResponseState::default()));
        let peer = spawn_peer(state.clone()).await;

        let attempt_id = "3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b";
        let mut ctx = VkDynamicToolContext::new(std::env::temp_dir());
        ctx.attempt_id = Some(attempt_id.to_string());

        let client = AppServerClient::new(
            LogWriter::new(tokio::io::sink()),
            Some(approvals.clone()),
            false,
            ctx,
        );

        let request = ServerRequest::DynamicToolCall {
            request_id: RequestId::Integer(1),
            params: DynamicToolCallParams {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                call_id: "call-1".to_string(),
                tool: crate::codex::dynamic_tools::VK_TOOL_TEST_MUTATING.to_string(),
                arguments: serde_json::json!({}),
            },
        };

        client
            .handle_server_request(&peer, request)
            .await
            .expect("handle request");

        let response = wait_for_response(state).await;
        let parsed: DynamicToolCallResponse =
            serde_json::from_value(response.result).expect("decode response");
        assert!(!parsed.success);
        assert!(matches!(
            parsed.content_items.first(),
            Some(DynamicToolCallOutputContentItem::InputText { text })
                if text.contains("requires approval")
        ));

        let calls = approvals.calls.lock().await.clone();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].0,
            crate::codex::dynamic_tools::VK_TOOL_TEST_MUTATING
        );
        assert_eq!(calls[0].1, "call-1");
    }
}

#[cfg(test)]
mod thread_response_lite_tests {
    use serde_json::json;

    #[test]
    fn thread_response_lite_decodes_when_thread_turn_items_evolve() {
        // Thread history evolves quickly (new items like `contextCompaction`, etc). We only need
        // the thread id, so decoding should not be coupled to the full `thread.turns` schema.
        let raw = json!({
            "thread": {
                "id": "thread_123",
                "turns": [{
                    "id": "turn_1",
                    "status": "completed",
                    "error": null,
                    "items": [{
                        "type": "futureThreadItem",
                        "id": "item_1",
                    }]
                }]
            },
            "model": "gpt-4.1",
            "modelProvider": "openai",
            "cwd": "/tmp",
            "approvalPolicy": "never",
            "sandbox": { "type": "dangerFullAccess" },
            "reasoningEffort": null
        });
        let parsed: super::ThreadResponseLite =
            serde_json::from_value(raw).expect("lite decode should succeed");
        assert_eq!(parsed.thread.id, "thread_123");
    }
}
