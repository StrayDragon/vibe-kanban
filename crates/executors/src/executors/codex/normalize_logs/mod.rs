use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use codex_app_server_protocol::{
    JSONRPCNotification, JSONRPCResponse, NewConversationResponse, ServerNotification,
};
use codex_mcp_types::ContentBlock;
use codex_protocol::{
    openai_models::ReasoningEffort,
    plan_tool::{StepStatus, UpdatePlanArgs},
    protocol::{
        AgentMessageDeltaEvent, AgentMessageEvent, AgentReasoningDeltaEvent, AgentReasoningEvent,
        AgentReasoningSectionBreakEvent, ApplyPatchApprovalRequestEvent, BackgroundEventEvent,
        ErrorEvent, EventMsg, ExecApprovalRequestEvent, ExecCommandBeginEvent, ExecCommandEndEvent,
        ExecCommandOutputDeltaEvent, ExecOutputStream, FileChange as CodexProtoFileChange,
        McpInvocation, McpToolCallBeginEvent, McpToolCallEndEvent, PatchApplyBeginEvent,
        PatchApplyEndEvent, StreamErrorEvent, ThreadRolledBackEvent, TokenUsageInfo,
        ViewImageToolCallEvent, WarningEvent, WebSearchBeginEvent, WebSearchEndEvent,
    },
};
use futures::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use workspace_utils::{
    approvals::ApprovalStatus, diff::normalize_unified_diff, msg_store::MsgStore,
    path::make_path_relative,
};

use crate::{
    approvals::ToolCallMetadata,
    executors::codex::session::SessionHandler,
    logs::{
        ActionType, CommandExitStatus, CommandRunResult, FileChange, NormalizedEntry,
        NormalizedEntryError, NormalizedEntryType, TodoItem, ToolResult, ToolResultValueType,
        ToolStatus,
        plain_text_processor::PlainTextLogProcessor,
        utils::{
            ConversationPatch, EntryIndexProvider,
            patch::{add_normalized_entry, replace_normalized_entry, upsert_normalized_entry},
        },
    },
};

trait ToNormalizedEntry {
    fn to_normalized_entry(&self) -> NormalizedEntry;
}

trait ToNormalizedEntryOpt {
    fn to_normalized_entry_opt(&self) -> Option<NormalizedEntry>;
}

#[derive(Debug, Deserialize)]
struct CodexNotificationParams {
    #[serde(rename = "msg")]
    msg: EventMsg,
}

#[derive(Default)]
struct StreamingText {
    index: usize,
    content: String,
}

#[derive(Default)]
struct CommandState {
    index: Option<usize>,
    command: String,
    stdout: String,
    stderr: String,
    formatted_output: Option<String>,
    status: ToolStatus,
    exit_code: Option<i32>,
    awaiting_approval: bool,
    call_id: String,
}

impl ToNormalizedEntry for CommandState {
    fn to_normalized_entry(&self) -> NormalizedEntry {
        let content = self.command.to_string();

        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "bash".to_string(),
                action_type: ActionType::CommandRun {
                    command: self.command.clone(),
                    result: Some(CommandRunResult {
                        exit_status: self
                            .exit_code
                            .map(|code| CommandExitStatus::ExitCode { code }),
                        output: if self.formatted_output.is_some() {
                            self.formatted_output.clone()
                        } else {
                            build_command_output(Some(&self.stdout), Some(&self.stderr))
                        },
                    }),
                },
                status: self.status.clone(),
            },
            content,
            metadata: serde_json::to_value(ToolCallMetadata {
                tool_call_id: self.call_id.clone(),
            })
            .ok(),
        }
    }
}

struct McpToolState {
    index: Option<usize>,
    invocation: McpInvocation,
    result: Option<ToolResult>,
    status: ToolStatus,
}

impl ToNormalizedEntry for McpToolState {
    fn to_normalized_entry(&self) -> NormalizedEntry {
        let tool_name = format!("mcp:{}:{}", self.invocation.server, self.invocation.tool);
        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: tool_name.clone(),
                action_type: ActionType::Tool {
                    tool_name,
                    arguments: self.invocation.arguments.clone(),
                    result: self.result.clone(),
                },
                status: self.status.clone(),
            },
            content: self.invocation.tool.clone(),
            metadata: None,
        }
    }
}

#[derive(Default)]
struct WebSearchState {
    index: Option<usize>,
    query: Option<String>,
    status: ToolStatus,
}

impl WebSearchState {
    fn new() -> Self {
        Default::default()
    }
}

impl ToNormalizedEntry for WebSearchState {
    fn to_normalized_entry(&self) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "web_search".to_string(),
                action_type: ActionType::WebFetch {
                    url: self.query.clone().unwrap_or_else(|| "...".to_string()),
                },
                status: self.status.clone(),
            },
            content: self
                .query
                .clone()
                .unwrap_or_else(|| "Web search".to_string()),
            metadata: None,
        }
    }
}

#[derive(Default)]
struct PatchState {
    entries: Vec<PatchEntry>,
}

struct PatchEntry {
    index: Option<usize>,
    path: String,
    changes: Vec<FileChange>,
    status: ToolStatus,
    awaiting_approval: bool,
    call_id: String,
}

impl ToNormalizedEntry for PatchEntry {
    fn to_normalized_entry(&self) -> NormalizedEntry {
        let content = self.path.clone();

        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ToolUse {
                tool_name: "edit".to_string(),
                action_type: ActionType::FileEdit {
                    path: self.path.clone(),
                    changes: self.changes.clone(),
                },
                status: self.status.clone(),
            },
            content,
            metadata: serde_json::to_value(ToolCallMetadata {
                tool_call_id: self.call_id.clone(),
            })
            .ok(),
        }
    }
}

struct LogState {
    entry_index: EntryIndexProvider,
    assistant: Option<StreamingText>,
    thinking: Option<StreamingText>,
    commands: HashMap<String, CommandState>,
    mcp_tools: HashMap<String, McpToolState>,
    patches: HashMap<String, PatchState>,
    web_searches: HashMap<String, WebSearchState>,
    token_usage_info: Option<TokenUsageInfo>,
}

enum StreamingTextKind {
    Assistant,
    Thinking,
}

impl LogState {
    fn new(entry_index: EntryIndexProvider) -> Self {
        Self {
            entry_index,
            assistant: None,
            thinking: None,
            commands: HashMap::new(),
            mcp_tools: HashMap::new(),
            patches: HashMap::new(),
            web_searches: HashMap::new(),
            token_usage_info: None,
        }
    }

    fn streaming_text_update(
        &mut self,
        content: String,
        type_: StreamingTextKind,
        mode: UpdateMode,
    ) -> (NormalizedEntry, usize, bool) {
        let index_provider = &self.entry_index;
        let entry = match type_ {
            StreamingTextKind::Assistant => &mut self.assistant,
            StreamingTextKind::Thinking => &mut self.thinking,
        };
        let is_new = entry.is_none();
        let (content, index) = if entry.is_none() {
            let index = index_provider.next();
            *entry = Some(StreamingText { index, content });
            (&entry.as_ref().unwrap().content, index)
        } else {
            let streaming_state = entry.as_mut().unwrap();
            match mode {
                UpdateMode::Append => streaming_state.content.push_str(&content),
                UpdateMode::Set => streaming_state.content = content,
            }
            (&streaming_state.content, streaming_state.index)
        };
        let normalized_entry = NormalizedEntry {
            timestamp: None,
            entry_type: match type_ {
                StreamingTextKind::Assistant => NormalizedEntryType::AssistantMessage,
                StreamingTextKind::Thinking => NormalizedEntryType::Thinking,
            },
            content: content.clone(),
            metadata: None,
        };
        (normalized_entry, index, is_new)
    }

    fn streaming_text_append(
        &mut self,
        content: String,
        type_: StreamingTextKind,
    ) -> (NormalizedEntry, usize, bool) {
        self.streaming_text_update(content, type_, UpdateMode::Append)
    }

    fn streaming_text_set(
        &mut self,
        content: String,
        type_: StreamingTextKind,
    ) -> (NormalizedEntry, usize, bool) {
        self.streaming_text_update(content, type_, UpdateMode::Set)
    }

    fn assistant_message_append(&mut self, content: String) -> (NormalizedEntry, usize, bool) {
        self.streaming_text_append(content, StreamingTextKind::Assistant)
    }

    fn thinking_append(&mut self, content: String) -> (NormalizedEntry, usize, bool) {
        self.streaming_text_append(content, StreamingTextKind::Thinking)
    }

    fn assistant_message(&mut self, content: String) -> (NormalizedEntry, usize, bool) {
        self.streaming_text_set(content, StreamingTextKind::Assistant)
    }

    fn thinking(&mut self, content: String) -> (NormalizedEntry, usize, bool) {
        self.streaming_text_set(content, StreamingTextKind::Thinking)
    }
}

enum UpdateMode {
    Append,
    Set,
}

fn normalize_file_changes(
    worktree_path: &str,
    changes: &HashMap<PathBuf, CodexProtoFileChange>,
) -> Vec<(String, Vec<FileChange>)> {
    changes
        .iter()
        .map(|(path, change)| {
            let path_str = path.to_string_lossy();
            let relative = make_path_relative(path_str.as_ref(), worktree_path);
            let file_changes = match change {
                CodexProtoFileChange::Add { content } => vec![FileChange::Write {
                    content: content.clone(),
                }],
                CodexProtoFileChange::Delete { .. } => vec![FileChange::Delete],
                CodexProtoFileChange::Update {
                    unified_diff,
                    move_path,
                } => {
                    let mut edits = Vec::new();
                    if let Some(dest) = move_path {
                        let dest_rel =
                            make_path_relative(dest.to_string_lossy().as_ref(), worktree_path);
                        edits.push(FileChange::Rename { new_path: dest_rel });
                    }
                    let diff = normalize_unified_diff(&relative, unified_diff);
                    edits.push(FileChange::Edit {
                        unified_diff: diff,
                        has_line_numbers: true,
                    });
                    edits
                }
            };
            (relative, file_changes)
        })
        .collect()
}

fn format_todo_status(status: &StepStatus) -> String {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::InProgress => "in_progress",
        StepStatus::Completed => "completed",
    }
    .to_string()
}

fn emit_normalization_error(
    msg_store: &Arc<MsgStore>,
    entry_index: &EntryIndexProvider,
    call_id: Option<&str>,
    message: impl Into<String>,
) {
    let content = match call_id {
        Some(call_id) => format!("Normalization error ({call_id}): {}", message.into()),
        None => format!("Normalization error: {}", message.into()),
    };

    add_normalized_entry(
        msg_store,
        entry_index,
        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::ErrorMessage {
                error_type: NormalizedEntryError::Other,
            },
            content,
            metadata: call_id.and_then(|call_id| {
                serde_json::to_value(ToolCallMetadata {
                    tool_call_id: call_id.to_string(),
                })
                .ok()
            }),
        },
    );
}

pub fn normalize_logs(msg_store: Arc<MsgStore>, worktree_path: &Path) {
    let entry_index = EntryIndexProvider::start_from(&msg_store);
    normalize_codex_stderr_logs(msg_store.clone(), entry_index.clone());

    let worktree_path_str = worktree_path.to_string_lossy().to_string();
    tokio::spawn(async move {
        let mut state = LogState::new(entry_index.clone());
        let mut stdout_lines = msg_store.stdout_lines_stream();

        while let Some(Ok(line)) = stdout_lines.next().await {
            if let Ok(error) = serde_json::from_str::<Error>(&line) {
                add_normalized_entry(&msg_store, &entry_index, error.to_normalized_entry());
                continue;
            }

            if let Ok(approval) = serde_json::from_str::<Approval>(&line) {
                if let Some(entry) = approval.to_normalized_entry_opt() {
                    add_normalized_entry(&msg_store, &entry_index, entry);
                }
                continue;
            }

            if let Ok(response) = serde_json::from_str::<JSONRPCResponse>(&line) {
                handle_jsonrpc_response(response, &msg_store, &entry_index);
                continue;
            }

            if let Ok(server_notification) = serde_json::from_str::<ServerNotification>(&line) {
                if let ServerNotification::SessionConfigured(session_configured) =
                    server_notification
                {
                    msg_store.push_session_id(session_configured.session_id.to_string());
                    handle_model_params(
                        session_configured.model,
                        session_configured.reasoning_effort,
                        &msg_store,
                        &entry_index,
                    );
                };
                continue;
            } else if let Some(session_id) = line
                .strip_prefix(r#"{"method":"sessionConfigured","params":{"sessionId":""#)
                .and_then(|suffix| SESSION_ID.captures(suffix).and_then(|caps| caps.get(1)))
            {
                // Best-effort extraction of session ID from logs in case the JSON parsing fails.
                // This could happen if the line is truncated due to size limits because it includes the full session history.
                msg_store.push_session_id(session_id.as_str().to_string());
                continue;
            }

            let notification: JSONRPCNotification = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(_) => continue,
            };

            if !notification.method.starts_with("codex/event") {
                continue;
            }

            let Some(params) = notification
                .params
                .and_then(|p| serde_json::from_value::<CodexNotificationParams>(p).ok())
            else {
                continue;
            };

            let event = params.msg;
            match event {
                EventMsg::SessionConfigured(payload) => {
                    msg_store.push_session_id(payload.session_id.to_string());
                    handle_model_params(
                        payload.model,
                        payload.reasoning_effort,
                        &msg_store,
                        &entry_index,
                    );
                }
                EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta }) => {
                    state.thinking = None;
                    let (entry, index, is_new) = state.assistant_message_append(delta);
                    upsert_normalized_entry(&msg_store, index, entry, is_new);
                }
                EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta }) => {
                    state.assistant = None;
                    let (entry, index, is_new) = state.thinking_append(delta);
                    upsert_normalized_entry(&msg_store, index, entry, is_new);
                }
                EventMsg::AgentMessage(AgentMessageEvent { message }) => {
                    state.thinking = None;
                    let (entry, index, is_new) = state.assistant_message(message);
                    upsert_normalized_entry(&msg_store, index, entry, is_new);
                    state.assistant = None;
                }
                EventMsg::AgentReasoning(AgentReasoningEvent { text }) => {
                    state.assistant = None;
                    let (entry, index, is_new) = state.thinking(text);
                    upsert_normalized_entry(&msg_store, index, entry, is_new);
                    state.thinking = None;
                }
                EventMsg::AgentReasoningSectionBreak(AgentReasoningSectionBreakEvent {
                    item_id: _,
                    summary_index: _,
                }) => {
                    state.assistant = None;
                    state.thinking = None;
                }
                EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
                    call_id,
                    turn_id: _,
                    command,
                    cwd: _,
                    reason,
                    parsed_cmd: _,
                    proposed_execpolicy_amendment: _,
                }) => {
                    state.assistant = None;
                    state.thinking = None;

                    let command_text = if command.is_empty() {
                        reason
                            .filter(|r| !r.is_empty())
                            .unwrap_or_else(|| "command execution".to_string())
                    } else {
                        command.join(" ")
                    };

                    let command_state = state.commands.entry(call_id.clone()).or_default();

                    if command_state.command.is_empty() {
                        command_state.command = command_text;
                    }
                    command_state.awaiting_approval = true;
                    if let Some(index) = command_state.index {
                        replace_normalized_entry(
                            &msg_store,
                            index,
                            command_state.to_normalized_entry(),
                        );
                    } else {
                        let index = add_normalized_entry(
                            &msg_store,
                            &entry_index,
                            command_state.to_normalized_entry(),
                        );
                        command_state.index = Some(index);
                    }
                }
                EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                    call_id,
                    turn_id: _,
                    changes,
                    reason: _,
                    grant_root: _,
                }) => {
                    state.assistant = None;
                    state.thinking = None;

                    let normalized = normalize_file_changes(&worktree_path_str, &changes);
                    let patch_state = state.patches.entry(call_id.clone()).or_default();

                    for entry in patch_state.entries.drain(..) {
                        if let Some(index) = entry.index {
                            msg_store.push_patch(ConversationPatch::remove(index));
                        }
                    }

                    for (path, file_changes) in normalized {
                        let mut entry = PatchEntry {
                            index: None,
                            path,
                            changes: file_changes,
                            status: ToolStatus::Created,
                            awaiting_approval: true,
                            call_id: call_id.clone(),
                        };
                        let index = add_normalized_entry(
                            &msg_store,
                            &entry_index,
                            entry.to_normalized_entry(),
                        );
                        entry.index = Some(index);
                        patch_state.entries.push(entry);
                    }
                }
                EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                    call_id,
                    turn_id: _,
                    command,
                    cwd: _,
                    parsed_cmd: _,
                    source: _,
                    interaction_input: _,
                    process_id: _,
                }) => {
                    state.assistant = None;
                    state.thinking = None;
                    let command_text = command.join(" ");
                    if command_text.is_empty() {
                        continue;
                    }
                    let mut command_state = CommandState {
                        index: None,
                        command: command_text,
                        stdout: String::new(),
                        stderr: String::new(),
                        formatted_output: None,
                        status: ToolStatus::Created,
                        exit_code: None,
                        awaiting_approval: false,
                        call_id: call_id.clone(),
                    };
                    let index = add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        command_state.to_normalized_entry(),
                    );
                    command_state.index = Some(index);
                    state.commands.insert(call_id, command_state);
                }
                EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                    call_id,
                    stream,
                    chunk,
                }) => {
                    if let Some(command_state) = state.commands.get_mut(&call_id) {
                        let chunk = String::from_utf8_lossy(&chunk);
                        if chunk.is_empty() {
                            continue;
                        }
                        match stream {
                            ExecOutputStream::Stdout => command_state.stdout.push_str(&chunk),
                            ExecOutputStream::Stderr => command_state.stderr.push_str(&chunk),
                        }
                        let Some(index) = command_state.index else {
                            tracing::error!(
                                call_id = %call_id,
                                "missing entry index for existing command state"
                            );
                            emit_normalization_error(
                                &msg_store,
                                &entry_index,
                                Some(&call_id),
                                "missing entry index for command output delta",
                            );
                            continue;
                        };
                        replace_normalized_entry(
                            &msg_store,
                            index,
                            command_state.to_normalized_entry(),
                        );
                    }
                }
                EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                    call_id,
                    turn_id: _,
                    command: _,
                    cwd: _,
                    parsed_cmd: _,
                    source: _,
                    interaction_input: _,
                    stdout: _,
                    stderr: _,
                    aggregated_output: _,
                    exit_code,
                    duration: _,
                    formatted_output,
                    process_id: _,
                }) => match state.commands.remove(&call_id) {
                    Some(mut command_state) => {
                        command_state.formatted_output = Some(formatted_output);
                        command_state.exit_code = Some(exit_code);
                        command_state.awaiting_approval = false;
                        command_state.status = if exit_code == 0 {
                            ToolStatus::Success
                        } else {
                            ToolStatus::Failed
                        };
                        let Some(index) = command_state.index else {
                            tracing::error!(
                                call_id = %call_id,
                                "missing entry index for existing command state"
                            );
                            emit_normalization_error(
                                &msg_store,
                                &entry_index,
                                Some(&call_id),
                                "missing entry index for command end",
                            );
                            continue;
                        };
                        replace_normalized_entry(
                            &msg_store,
                            index,
                            command_state.to_normalized_entry(),
                        );
                    }
                    None => {
                        tracing::warn!(
                            call_id = %call_id,
                            "received ExecCommandEnd without matching command state"
                        );
                        emit_normalization_error(
                            &msg_store,
                            &entry_index,
                            Some(&call_id),
                            "ExecCommandEnd without matching command state",
                        );
                    }
                },
                EventMsg::BackgroundEvent(BackgroundEventEvent { message }) => {
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::SystemMessage,
                            content: format!("Background event: {message}"),
                            metadata: None,
                        },
                    );
                }
                EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns }) => {
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::SystemMessage,
                            content: format!("Thread rolled back ({num_turns} turns)"),
                            metadata: None,
                        },
                    );
                }
                EventMsg::StreamError(StreamErrorEvent {
                    message,
                    codex_error_info,
                    additional_details,
                }) => {
                    let mut content = format!("Stream error: {message}");
                    if let Some(details) = additional_details {
                        content.push_str(&format!(" Details: {details}"));
                    }
                    if let Some(info) = codex_error_info {
                        content.push_str(&format!(" {info:?}"));
                    }
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ErrorMessage {
                                error_type: NormalizedEntryError::Other,
                            },
                            content,
                            metadata: None,
                        },
                    );
                }
                EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                    call_id,
                    invocation,
                }) => {
                    state.assistant = None;
                    state.thinking = None;
                    let mut mcp_tool_state = McpToolState {
                        index: None,
                        invocation,
                        result: None,
                        status: ToolStatus::Created,
                    };
                    let index = add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        mcp_tool_state.to_normalized_entry(),
                    );
                    mcp_tool_state.index = Some(index);
                    state.mcp_tools.insert(call_id, mcp_tool_state);
                }
                EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                    call_id, result, ..
                }) => match state.mcp_tools.remove(&call_id) {
                    Some(mut mcp_tool_state) => {
                        match result {
                            Ok(value) => {
                                mcp_tool_state.status = if value.is_error.unwrap_or(false) {
                                    ToolStatus::Failed
                                } else {
                                    ToolStatus::Success
                                };
                                if value
                                    .content
                                    .iter()
                                    .all(|block| matches!(block, ContentBlock::TextContent(_)))
                                {
                                    mcp_tool_state.result = Some(ToolResult {
                                        r#type: ToolResultValueType::Markdown,
                                        value: Value::String(
                                            value
                                                .content
                                                .iter()
                                                .map(|block| {
                                                    if let ContentBlock::TextContent(content) =
                                                        block
                                                    {
                                                        content.text.clone()
                                                    } else {
                                                        unreachable!()
                                                    }
                                                })
                                                .collect::<Vec<String>>()
                                                .join("\n"),
                                        ),
                                    });
                                } else {
                                    mcp_tool_state.result = Some(ToolResult {
                                        r#type: ToolResultValueType::Json,
                                        value: value.structured_content.unwrap_or_else(|| {
                                            serde_json::to_value(value.content).unwrap_or_default()
                                        }),
                                    });
                                }
                            }
                            Err(err) => {
                                mcp_tool_state.status = ToolStatus::Failed;
                                mcp_tool_state.result = Some(ToolResult {
                                    r#type: ToolResultValueType::Markdown,
                                    value: Value::String(err),
                                });
                            }
                        };
                        let Some(index) = mcp_tool_state.index else {
                            tracing::error!(
                                call_id = %call_id,
                                "missing entry index for existing mcp tool state"
                            );
                            emit_normalization_error(
                                &msg_store,
                                &entry_index,
                                Some(&call_id),
                                "missing entry index for MCP tool call end",
                            );
                            continue;
                        };
                        replace_normalized_entry(
                            &msg_store,
                            index,
                            mcp_tool_state.to_normalized_entry(),
                        );
                    }
                    None => {
                        tracing::warn!(
                            call_id = %call_id,
                            "received McpToolCallEnd without matching mcp tool state"
                        );
                        emit_normalization_error(
                            &msg_store,
                            &entry_index,
                            Some(&call_id),
                            "McpToolCallEnd without matching tool call state",
                        );
                    }
                },
                EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                    call_id, changes, ..
                }) => {
                    state.assistant = None;
                    state.thinking = None;
                    let normalized = normalize_file_changes(&worktree_path_str, &changes);
                    if let Some(patch_state) = state.patches.get_mut(&call_id) {
                        let mut iter = normalized.into_iter();
                        for entry in &mut patch_state.entries {
                            if let Some((path, file_changes)) = iter.next() {
                                entry.path = path;
                                entry.changes = file_changes;
                            }
                            entry.status = ToolStatus::Created;
                            entry.awaiting_approval = false;
                            if let Some(index) = entry.index {
                                replace_normalized_entry(
                                    &msg_store,
                                    index,
                                    entry.to_normalized_entry(),
                                );
                            } else {
                                let index = add_normalized_entry(
                                    &msg_store,
                                    &entry_index,
                                    entry.to_normalized_entry(),
                                );
                                entry.index = Some(index);
                            }
                        }
                        for (path, file_changes) in iter {
                            let mut entry = PatchEntry {
                                index: None,
                                path,
                                changes: file_changes,
                                status: ToolStatus::Created,
                                awaiting_approval: false,
                                call_id: call_id.clone(),
                            };
                            let index = add_normalized_entry(
                                &msg_store,
                                &entry_index,
                                entry.to_normalized_entry(),
                            );
                            entry.index = Some(index);
                            patch_state.entries.push(entry);
                        }
                    } else {
                        let mut patch_state = PatchState::default();
                        for (path, file_changes) in normalized {
                            let mut patch_entry = PatchEntry {
                                index: None,
                                path,
                                changes: file_changes,
                                status: ToolStatus::Created,
                                awaiting_approval: false,
                                call_id: call_id.clone(),
                            };
                            let index = add_normalized_entry(
                                &msg_store,
                                &entry_index,
                                patch_entry.to_normalized_entry(),
                            );
                            patch_entry.index = Some(index);
                            patch_state.entries.push(patch_entry);
                        }
                        state.patches.insert(call_id, patch_state);
                    }
                }
                EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                    call_id,
                    stdout: _,
                    stderr: _,
                    success,
                    ..
                }) => match state.patches.remove(&call_id) {
                    Some(patch_state) => {
                        let status = if success {
                            ToolStatus::Success
                        } else {
                            ToolStatus::Failed
                        };
                        for mut entry in patch_state.entries {
                            entry.status = status.clone();
                            let Some(index) = entry.index else {
                                tracing::error!(
                                    call_id = %call_id,
                                    "missing entry index for existing patch entry"
                                );
                                emit_normalization_error(
                                    &msg_store,
                                    &entry_index,
                                    Some(&call_id),
                                    "missing entry index for patch apply end",
                                );
                                continue;
                            };
                            replace_normalized_entry(
                                &msg_store,
                                index,
                                entry.to_normalized_entry(),
                            );
                        }
                    }
                    None => {
                        tracing::warn!(
                            call_id = %call_id,
                            "received PatchApplyEnd without matching patch state"
                        );
                        emit_normalization_error(
                            &msg_store,
                            &entry_index,
                            Some(&call_id),
                            "PatchApplyEnd without matching patch state",
                        );
                    }
                },
                EventMsg::WebSearchBegin(WebSearchBeginEvent { call_id }) => {
                    state.assistant = None;
                    state.thinking = None;
                    let mut web_search_state = WebSearchState::new();
                    let index = add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        web_search_state.to_normalized_entry(),
                    );
                    web_search_state.index = Some(index);
                    state.web_searches.insert(call_id, web_search_state);
                }
                EventMsg::WebSearchEnd(WebSearchEndEvent { call_id, query }) => {
                    state.assistant = None;
                    state.thinking = None;
                    match state.web_searches.remove(&call_id) {
                        Some(mut entry) => {
                            entry.status = ToolStatus::Success;
                            entry.query = Some(query.clone());
                            let normalized_entry = entry.to_normalized_entry();
                            let Some(index) = entry.index else {
                                tracing::error!(
                                    call_id = %call_id,
                                    "missing entry index for existing websearch entry"
                                );
                                emit_normalization_error(
                                    &msg_store,
                                    &entry_index,
                                    Some(&call_id),
                                    "missing entry index for web search end",
                                );
                                continue;
                            };
                            replace_normalized_entry(&msg_store, index, normalized_entry);
                        }
                        None => {
                            tracing::warn!(
                                call_id = %call_id,
                                "received WebSearchEnd without matching web search state"
                            );
                            emit_normalization_error(
                                &msg_store,
                                &entry_index,
                                Some(&call_id),
                                "WebSearchEnd without matching web search state",
                            );
                        }
                    }
                }
                EventMsg::ViewImageToolCall(ViewImageToolCallEvent { call_id: _, path }) => {
                    state.assistant = None;
                    state.thinking = None;
                    let path_str = path.to_string_lossy().to_string();
                    let relative_path = make_path_relative(&path_str, &worktree_path_str);
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ToolUse {
                                tool_name: "view_image".to_string(),
                                action_type: ActionType::FileRead {
                                    path: relative_path.clone(),
                                },
                                status: ToolStatus::Success,
                            },
                            content: relative_path.to_string(),
                            metadata: None,
                        },
                    );
                }
                EventMsg::PlanUpdate(UpdatePlanArgs { plan, explanation }) => {
                    let todos: Vec<TodoItem> = plan
                        .iter()
                        .map(|item| TodoItem {
                            content: item.step.clone(),
                            status: format_todo_status(&item.status),
                            priority: None,
                        })
                        .collect();
                    let explanation = explanation
                        .as_ref()
                        .map(|text| text.trim())
                        .filter(|text| !text.is_empty())
                        .map(|text| text.to_string());
                    let content = explanation.clone().unwrap_or_else(|| {
                        if todos.is_empty() {
                            "Plan updated".to_string()
                        } else {
                            format!("Plan updated ({} steps)", todos.len())
                        }
                    });

                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ToolUse {
                                tool_name: "plan".to_string(),
                                action_type: ActionType::TodoManagement {
                                    todos,
                                    operation: "update".to_string(),
                                },
                                status: ToolStatus::Success,
                            },
                            content,
                            metadata: None,
                        },
                    );
                }
                EventMsg::Warning(WarningEvent { message }) => {
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ErrorMessage {
                                error_type: NormalizedEntryError::Other,
                            },
                            content: message,
                            metadata: None,
                        },
                    );
                }
                EventMsg::Error(ErrorEvent {
                    message,
                    codex_error_info,
                }) => {
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ErrorMessage {
                                error_type: NormalizedEntryError::Other,
                            },
                            content: format!("Error: {message} {codex_error_info:?}"),
                            metadata: None,
                        },
                    );
                }
                EventMsg::TokenCount(payload) => {
                    if let Some(info) = payload.info {
                        state.token_usage_info = Some(info);
                    }
                }
                EventMsg::ContextCompacted(..) => {
                    add_normalized_entry(
                        &msg_store,
                        &entry_index,
                        NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::SystemMessage,
                            content: "Context compacted".to_string(),
                            metadata: None,
                        },
                    );
                }
                EventMsg::AgentReasoningRawContent(..)
                | EventMsg::AgentReasoningRawContentDelta(..)
                | EventMsg::TurnStarted(..)
                | EventMsg::UserMessage(..)
                | EventMsg::TurnDiff(..)
                | EventMsg::GetHistoryEntryResponse(..)
                | EventMsg::McpListToolsResponse(..)
                | EventMsg::McpStartupComplete(..)
                | EventMsg::McpStartupUpdate(..)
                | EventMsg::DeprecationNotice(..)
                | EventMsg::UndoCompleted(..)
                | EventMsg::UndoStarted(..)
                | EventMsg::RawResponseItem(..)
                | EventMsg::ItemStarted(..)
                | EventMsg::ItemCompleted(..)
                | EventMsg::AgentMessageContentDelta(..)
                | EventMsg::ReasoningContentDelta(..)
                | EventMsg::ReasoningRawContentDelta(..)
                | EventMsg::ListCustomPromptsResponse(..)
                | EventMsg::TurnAborted(..)
                | EventMsg::ShutdownComplete
                | EventMsg::EnteredReviewMode(..)
                | EventMsg::ExitedReviewMode(..)
                | EventMsg::TerminalInteraction(..)
                | EventMsg::ElicitationRequest(..)
                | EventMsg::TurnComplete(..) => {}
                _ => {}
            }
        }
    });
}

fn normalize_codex_stderr_logs(msg_store: Arc<MsgStore>, entry_index_provider: EntryIndexProvider) {
    tokio::spawn(async move {
        let mut stderr = msg_store.stderr_chunked_stream();

        let mut processor = PlainTextLogProcessor::builder()
            .normalized_entry_producer(Box::new(|content: String| NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::SystemMessage,
                content: strip_ansi_escapes::strip_str(&content),
                metadata: None,
            }))
            .transform_lines(Box::new(|lines| {
                lines.retain(|line| {
                    // codex-core 0.84 logs this at ERROR even when it's benign.
                    !line.contains("codex_core::codex: needs_follow_up:")
                });
            }))
            .time_gap(std::time::Duration::from_secs(2))
            .index_provider(entry_index_provider)
            .build();

        while let Some(Ok(chunk)) = stderr.next().await {
            for patch in processor.process(chunk) {
                msg_store.push_patch(patch);
            }
        }
    });
}

fn handle_jsonrpc_response(
    response: JSONRPCResponse,
    msg_store: &Arc<MsgStore>,
    entry_index: &EntryIndexProvider,
) {
    let Ok(response) = serde_json::from_value::<NewConversationResponse>(response.result.clone())
    else {
        return;
    };

    match SessionHandler::extract_session_id_from_rollout_path(response.rollout_path) {
        Ok(session_id) => msg_store.push_session_id(session_id),
        Err(err) => tracing::error!("failed to extract session id: {err}"),
    }

    handle_model_params(
        response.model,
        response.reasoning_effort,
        msg_store,
        entry_index,
    );
}

fn handle_model_params(
    model: String,
    reasoning_effort: Option<ReasoningEffort>,
    msg_store: &Arc<MsgStore>,
    entry_index: &EntryIndexProvider,
) {
    let mut params = vec![];
    params.push(format!("model: {model}"));
    if let Some(reasoning_effort) = reasoning_effort {
        params.push(format!("reasoning effort: {reasoning_effort}"));
    }

    add_normalized_entry(
        msg_store,
        entry_index,
        NormalizedEntry {
            timestamp: None,
            entry_type: NormalizedEntryType::SystemMessage,
            content: params.join("  ").to_string(),
            metadata: None,
        },
    );
}

fn build_command_output(stdout: Option<&str>, stderr: Option<&str>) -> Option<String> {
    let mut sections = Vec::new();
    if let Some(out) = stdout {
        let cleaned = out.trim();
        if !cleaned.is_empty() {
            sections.push(format!("stdout:\n{cleaned}"));
        }
    }
    if let Some(err) = stderr {
        let cleaned = err.trim();
        if !cleaned.is_empty() {
            sections.push(format!("stderr:\n{cleaned}"));
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

static SESSION_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})"#)
        .expect("valid regex")
});

#[derive(Serialize, Deserialize, Debug)]
pub enum Error {
    LaunchError { error: String },
    AuthRequired { error: String },
}

impl Error {
    pub fn launch_error(error: String) -> Self {
        Self::LaunchError { error }
    }
    pub fn auth_required(error: String) -> Self {
        Self::AuthRequired { error }
    }

    pub fn raw(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

impl ToNormalizedEntry for Error {
    fn to_normalized_entry(&self) -> NormalizedEntry {
        match self {
            Error::LaunchError { error } => NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::ErrorMessage {
                    error_type: NormalizedEntryError::Other,
                },
                content: error.clone(),
                metadata: None,
            },
            Error::AuthRequired { error } => NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::ErrorMessage {
                    error_type: NormalizedEntryError::SetupRequired,
                },
                content: error.clone(),
                metadata: None,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Approval {
    ApprovalResponse {
        call_id: String,
        tool_name: String,
        approval_status: ApprovalStatus,
    },
}

impl Approval {
    pub fn approval_response(
        call_id: String,
        tool_name: String,
        approval_status: ApprovalStatus,
    ) -> Self {
        Self::ApprovalResponse {
            call_id,
            tool_name,
            approval_status,
        }
    }

    pub fn raw(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn display_tool_name(&self) -> String {
        let Self::ApprovalResponse { tool_name, .. } = self;
        match tool_name.as_str() {
            "codex.exec_command" => "Exec Command".to_string(),
            "codex.apply_patch" => "Edit".to_string(),
            other => other.to_string(),
        }
    }
}

impl ToNormalizedEntryOpt for Approval {
    fn to_normalized_entry_opt(&self) -> Option<NormalizedEntry> {
        let Self::ApprovalResponse {
            call_id: _,
            tool_name: _,
            approval_status,
        } = self;
        let tool_name = self.display_tool_name();

        match approval_status {
            ApprovalStatus::Pending => None,
            ApprovalStatus::Approved => None,
            ApprovalStatus::Denied { reason } => Some(NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::UserFeedback {
                    denied_tool: tool_name.clone(),
                },
                content: reason
                    .clone()
                    .unwrap_or_else(|| "User denied this tool use request".to_string())
                    .trim()
                    .to_string(),
                metadata: None,
            }),
            ApprovalStatus::TimedOut => Some(NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::ErrorMessage {
                    error_type: NormalizedEntryError::Other,
                },
                content: format!("Approval timed out for tool {tool_name}"),
                metadata: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

    use codex_app_server_protocol::{
        JSONRPCNotification, JSONRPCResponse, NewConversationResponse, RequestId,
    };
    use codex_mcp_types::{CallToolResult, ContentBlock, ImageContent, TextContent};
    use codex_protocol::{
        ThreadId,
        plan_tool::{PlanItemArg, StepStatus, UpdatePlanArgs},
        protocol::{ExecCommandSource, FileChange as CodexProtoFileChange},
    };
    use serde_json::json;
    use tokio::time::{Instant, sleep};
    use workspace_utils::{approvals::ApprovalStatus, log_msg::LogMsg};

    use super::*;
    use crate::logs::{
        ActionType, CommandExitStatus, FileChange, NormalizedEntry, NormalizedEntryError,
        NormalizedEntryType, ToolResultValueType, ToolStatus,
    };

    fn push_json_line(msg_store: &Arc<MsgStore>, line: String) {
        msg_store.push_stdout(format!("{line}\n"));
    }

    fn push_codex_event(msg_store: &Arc<MsgStore>, msg: EventMsg) {
        let params = json!({ "msg": msg });
        let notification = JSONRPCNotification {
            method: "codex/event".to_string(),
            params: Some(params),
        };
        let line = serde_json::to_string(&notification).expect("notification");
        push_json_line(msg_store, line);
    }

    fn normalized_entries(msg_store: &MsgStore) -> Vec<NormalizedEntry> {
        let (entries, _) = msg_store.normalized_history_page(usize::MAX, None);
        entries
            .into_iter()
            .filter_map(|snapshot| snapshot.entry_json.get("content").cloned())
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect()
    }

    async fn wait_for_entry<F>(msg_store: &MsgStore, predicate: F) -> NormalizedEntry
    where
        F: Fn(&NormalizedEntry) -> bool,
    {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(entry) = normalized_entries(msg_store)
                .into_iter()
                .find(|entry| predicate(entry))
            {
                return entry;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for normalized entry");
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    #[test]
    fn build_command_output_returns_none_for_empty_sections() {
        assert_eq!(build_command_output(None, None), None);
        assert_eq!(build_command_output(Some(" \n"), Some("\n\t")), None);
    }

    #[test]
    fn build_command_output_formats_stdout_and_stderr() {
        let output = build_command_output(Some("ok\n"), Some("warn\n")).expect("expected output");
        assert_eq!(output, "stdout:\nok\n\nstderr:\nwarn");
    }

    #[test]
    fn approval_display_tool_name_maps_known_names() {
        let approval = Approval::approval_response(
            "call-1".to_string(),
            "codex.exec_command".to_string(),
            ApprovalStatus::Pending,
        );
        assert_eq!(approval.display_tool_name(), "Exec Command");

        let approval = Approval::approval_response(
            "call-2".to_string(),
            "codex.apply_patch".to_string(),
            ApprovalStatus::Pending,
        );
        assert_eq!(approval.display_tool_name(), "Edit");

        let approval = Approval::approval_response(
            "call-3".to_string(),
            "custom".to_string(),
            ApprovalStatus::Pending,
        );
        assert_eq!(approval.display_tool_name(), "custom");
    }

    #[test]
    fn approval_denied_emits_user_feedback() {
        let approval = Approval::approval_response(
            "call-4".to_string(),
            "codex.exec_command".to_string(),
            ApprovalStatus::Denied {
                reason: Some(" no ".to_string()),
            },
        );

        let entry = approval.to_normalized_entry_opt().expect("expected entry");
        match entry.entry_type {
            NormalizedEntryType::UserFeedback { denied_tool } => {
                assert_eq!(denied_tool, "Exec Command");
            }
            _ => panic!("expected user feedback entry"),
        }
        assert_eq!(entry.content, "no");
    }

    #[test]
    fn approval_timeout_emits_error_message() {
        let approval = Approval::approval_response(
            "call-5".to_string(),
            "custom".to_string(),
            ApprovalStatus::TimedOut,
        );

        let entry = approval.to_normalized_entry_opt().expect("expected entry");
        match entry.entry_type {
            NormalizedEntryType::ErrorMessage { error_type } => {
                assert_eq!(error_type, NormalizedEntryError::Other);
            }
            _ => panic!("expected error entry"),
        }
        assert_eq!(entry.content, "Approval timed out for tool custom");
    }

    #[test]
    fn normalize_file_changes_adds_write_and_delete() {
        let mut changes = HashMap::new();
        changes.insert(
            PathBuf::from("/repo/new.txt"),
            CodexProtoFileChange::Add {
                content: "hello".to_string(),
            },
        );
        changes.insert(
            PathBuf::from("/repo/old.txt"),
            CodexProtoFileChange::Delete {
                content: "old".to_string(),
            },
        );

        let results = normalize_file_changes("/repo", &changes);
        let mut by_path = HashMap::new();
        for (path, edits) in results {
            by_path.insert(path, edits);
        }

        let add_changes = by_path.get("new.txt").expect("add entry");
        match &add_changes[0] {
            FileChange::Write { content } => assert_eq!(content, "hello"),
            _ => panic!("expected write change"),
        }

        let delete_changes = by_path.get("old.txt").expect("delete entry");
        match &delete_changes[0] {
            FileChange::Delete => {}
            _ => panic!("expected delete change"),
        }
    }

    #[test]
    fn normalize_file_changes_handles_rename_and_edit() {
        let mut changes = HashMap::new();
        changes.insert(
            PathBuf::from("/repo/old.txt"),
            CodexProtoFileChange::Update {
                unified_diff: "@@ -1,1 +1,1 @@\n-old\n+new\n".to_string(),
                move_path: Some(PathBuf::from("/repo/new.txt")),
            },
        );

        let results = normalize_file_changes("/repo", &changes);
        assert_eq!(results.len(), 1);
        let (path, edits) = &results[0];
        assert_eq!(path, "old.txt");
        assert_eq!(edits.len(), 2);

        match &edits[0] {
            FileChange::Rename { new_path } => assert_eq!(new_path, "new.txt"),
            _ => panic!("expected rename change"),
        }

        match &edits[1] {
            FileChange::Edit {
                unified_diff,
                has_line_numbers,
            } => {
                assert!(unified_diff.contains("--- a/old.txt"));
                assert!(unified_diff.contains("+++ b/old.txt"));
                assert!(*has_line_numbers);
            }
            _ => panic!("expected edit change"),
        }
    }

    #[test]
    fn log_state_appends_assistant_messages() {
        let mut state = LogState::new(EntryIndexProvider::test_new());

        let (entry, first_index, first_new) = state.assistant_message_append("Hello".to_string());
        assert!(matches!(
            entry.entry_type,
            NormalizedEntryType::AssistantMessage
        ));
        assert!(first_new);

        let (entry, second_index, second_new) =
            state.assistant_message_append(" world".to_string());
        assert_eq!(first_index, second_index);
        assert!(!second_new);
        assert_eq!(entry.content, "Hello world");
    }

    #[tokio::test]
    async fn normalize_logs_exec_command_lifecycle() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        push_codex_event(
            &msg_store,
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: "cmd-1".to_string(),
                process_id: None,
                turn_id: "turn-1".to_string(),
                command: vec!["echo".to_string(), "hello".to_string()],
                cwd: PathBuf::from("/repo"),
                parsed_cmd: Vec::new(),
                source: ExecCommandSource::default(),
                interaction_input: None,
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: "cmd-1".to_string(),
                stream: ExecOutputStream::Stdout,
                chunk: b"hi\n".to_vec(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "cmd-1".to_string(),
                process_id: None,
                turn_id: "turn-1".to_string(),
                command: vec!["echo".to_string(), "hello".to_string()],
                cwd: PathBuf::from("/repo"),
                parsed_cmd: Vec::new(),
                source: ExecCommandSource::default(),
                interaction_input: None,
                stdout: "raw".to_string(),
                stderr: String::new(),
                aggregated_output: String::new(),
                exit_code: 0,
                duration: Duration::from_secs(1),
                formatted_output: "formatted output".to_string(),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::CommandRun { command, .. },
                status,
                ..
            } => command == "echo hello" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::CommandRun { command, result },
                status,
                ..
            } => {
                assert_eq!(entry.content, "echo hello");
                assert_eq!(command, "echo hello");
                assert!(matches!(status, ToolStatus::Success));
                let result = result.expect("command result");
                assert_eq!(result.output, Some("formatted output".to_string()));
                match result.exit_status {
                    Some(CommandExitStatus::ExitCode { code }) => assert_eq!(code, 0),
                    other => panic!("unexpected exit status: {other:?}"),
                }
            }
            _ => panic!("expected command tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_exec_command_failure_marks_failed() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        push_codex_event(
            &msg_store,
            EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
                call_id: "cmd-fail".to_string(),
                process_id: None,
                turn_id: "turn-1".to_string(),
                command: vec!["false".to_string()],
                cwd: PathBuf::from("/repo"),
                parsed_cmd: Vec::new(),
                source: ExecCommandSource::default(),
                interaction_input: None,
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "cmd-fail".to_string(),
                process_id: None,
                turn_id: "turn-1".to_string(),
                command: vec!["false".to_string()],
                cwd: PathBuf::from("/repo"),
                parsed_cmd: Vec::new(),
                source: ExecCommandSource::default(),
                interaction_input: None,
                stdout: String::new(),
                stderr: "nope".to_string(),
                aggregated_output: String::new(),
                exit_code: 2,
                duration: Duration::from_secs(1),
                formatted_output: "failed".to_string(),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::CommandRun { command, .. },
                status,
                ..
            } => command == "false" && matches!(status, ToolStatus::Failed),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::CommandRun { result, .. },
                status,
                ..
            } => {
                assert!(matches!(status, ToolStatus::Failed));
                let result = result.expect("command result");
                match result.exit_status {
                    Some(CommandExitStatus::ExitCode { code }) => assert_eq!(code, 2),
                    other => panic!("unexpected exit status: {other:?}"),
                }
            }
            _ => panic!("expected command tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_exec_command_end_without_begin_emits_normalization_error() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        push_codex_event(
            &msg_store,
            EventMsg::ExecCommandEnd(ExecCommandEndEvent {
                call_id: "cmd-missing".to_string(),
                process_id: None,
                turn_id: "turn-1".to_string(),
                command: vec!["echo".to_string(), "hello".to_string()],
                cwd: PathBuf::from("/repo"),
                parsed_cmd: Vec::new(),
                source: ExecCommandSource::default(),
                interaction_input: None,
                stdout: String::new(),
                stderr: String::new(),
                aggregated_output: String::new(),
                exit_code: 0,
                duration: Duration::from_secs(1),
                formatted_output: String::new(),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| {
            matches!(entry.entry_type, NormalizedEntryType::ErrorMessage { .. })
                && entry.content.contains("Normalization error (cmd-missing)")
                && entry
                    .content
                    .contains("ExecCommandEnd without matching command state")
        })
        .await;

        assert!(entry.content.contains("cmd-missing"));
        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_patch_apply_flow() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let mut changes = HashMap::new();
        changes.insert(
            PathBuf::from("/repo/file.txt"),
            CodexProtoFileChange::Update {
                unified_diff: "@@ -1,1 +1,1 @@\n-old\n+new\n".to_string(),
                move_path: None,
            },
        );

        push_codex_event(
            &msg_store,
            EventMsg::ApplyPatchApprovalRequest(ApplyPatchApprovalRequestEvent {
                call_id: "patch-1".to_string(),
                turn_id: "turn-1".to_string(),
                changes: changes.clone(),
                reason: None,
                grant_root: None,
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id: "patch-1".to_string(),
                turn_id: "turn-1".to_string(),
                auto_approved: false,
                changes: changes.clone(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: "patch-1".to_string(),
                turn_id: "turn-1".to_string(),
                stdout: String::new(),
                stderr: String::new(),
                success: true,
                changes: HashMap::new(),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "edit" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::FileEdit { path, changes },
                status,
                ..
            } => {
                assert_eq!(path, "file.txt");
                assert!(matches!(status, ToolStatus::Success));
                assert_eq!(changes.len(), 1);
                match &changes[0] {
                    FileChange::Edit {
                        unified_diff,
                        has_line_numbers,
                    } => {
                        assert!(unified_diff.contains("--- a/file.txt"));
                        assert!(unified_diff.contains("+++ b/file.txt"));
                        assert!(*has_line_numbers);
                    }
                    _ => panic!("expected edit change"),
                }
            }
            _ => panic!("expected patch tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_patch_apply_failure_marks_failed() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let mut changes = HashMap::new();
        changes.insert(
            PathBuf::from("/repo/bad.txt"),
            CodexProtoFileChange::Update {
                unified_diff: "@@ -1,1 +1,1 @@\n-old\n+new\n".to_string(),
                move_path: None,
            },
        );

        push_codex_event(
            &msg_store,
            EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
                call_id: "patch-fail".to_string(),
                turn_id: "turn-1".to_string(),
                auto_approved: true,
                changes,
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::PatchApplyEnd(PatchApplyEndEvent {
                call_id: "patch-fail".to_string(),
                turn_id: "turn-1".to_string(),
                stdout: String::new(),
                stderr: "bad diff".to_string(),
                success: false,
                changes: HashMap::new(),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "edit" && matches!(status, ToolStatus::Failed),
            _ => false,
        })
        .await;

        assert!(matches!(
            entry.entry_type,
            NormalizedEntryType::ToolUse {
                status: ToolStatus::Failed,
                ..
            }
        ));

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_mcp_tool_markdown_result() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let invocation = McpInvocation {
            server: "context7".to_string(),
            tool: "search".to_string(),
            arguments: Some(json!({"q": "rust"})),
        };

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: "mcp-1".to_string(),
                invocation: invocation.clone(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: "mcp-1".to_string(),
                invocation: invocation.clone(),
                duration: Duration::from_secs(1),
                result: Ok(CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        annotations: None,
                        text: "hello".to_string(),
                        r#type: "text".to_string(),
                    })],
                    is_error: None,
                    structured_content: None,
                }),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "mcp:context7:search" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name,
                action_type:
                    ActionType::Tool {
                        tool_name: action_tool,
                        arguments,
                        result,
                    },
                status,
            } => {
                assert_eq!(tool_name, "mcp:context7:search");
                assert_eq!(action_tool, "mcp:context7:search");
                assert_eq!(arguments, Some(json!({"q": "rust"})));
                assert!(matches!(status, ToolStatus::Success));
                let result = result.expect("tool result");
                assert!(matches!(result.r#type, ToolResultValueType::Markdown));
                assert_eq!(result.value, json!("hello"));
            }
            _ => panic!("expected mcp tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_mcp_tool_error_result_marks_failed() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let invocation = McpInvocation {
            server: "context7".to_string(),
            tool: "search".to_string(),
            arguments: None,
        };

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: "mcp-fail".to_string(),
                invocation: invocation.clone(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: "mcp-fail".to_string(),
                invocation: invocation.clone(),
                duration: Duration::from_secs(1),
                result: Err("boom".to_string()),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "mcp:context7:search" && matches!(status, ToolStatus::Failed),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::Tool { result, .. },
                status,
                ..
            } => {
                assert!(matches!(status, ToolStatus::Failed));
                let result = result.expect("tool result");
                assert!(matches!(result.r#type, ToolResultValueType::Markdown));
                assert_eq!(result.value, json!("boom"));
            }
            _ => panic!("expected mcp tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_mcp_tool_is_error_marks_failed() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let invocation = McpInvocation {
            server: "context7".to_string(),
            tool: "search".to_string(),
            arguments: None,
        };

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: "mcp-is-error".to_string(),
                invocation: invocation.clone(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: "mcp-is-error".to_string(),
                invocation: invocation.clone(),
                duration: Duration::from_secs(1),
                result: Ok(CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        annotations: None,
                        text: "bad result".to_string(),
                        r#type: "text".to_string(),
                    })],
                    is_error: Some(true),
                    structured_content: None,
                }),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "mcp:context7:search" && matches!(status, ToolStatus::Failed),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::Tool { result, .. },
                status,
                ..
            } => {
                assert!(matches!(status, ToolStatus::Failed));
                let result = result.expect("tool result");
                assert!(matches!(result.r#type, ToolResultValueType::Markdown));
                assert_eq!(result.value, json!("bad result"));
            }
            _ => panic!("expected mcp tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_mcp_tool_json_result() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let invocation = McpInvocation {
            server: "playwright".to_string(),
            tool: "screenshot".to_string(),
            arguments: None,
        };

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
                call_id: "mcp-2".to_string(),
                invocation: invocation.clone(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::McpToolCallEnd(McpToolCallEndEvent {
                call_id: "mcp-2".to_string(),
                invocation: invocation.clone(),
                duration: Duration::from_secs(1),
                result: Ok(CallToolResult {
                    content: vec![ContentBlock::ImageContent(ImageContent {
                        annotations: None,
                        data: "ZGF0YQ==".to_string(),
                        mime_type: "image/png".to_string(),
                        r#type: "image".to_string(),
                    })],
                    is_error: Some(false),
                    structured_content: Some(json!({"path": "out.png"})),
                }),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "mcp:playwright:screenshot" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::Tool { result, .. },
                status,
                ..
            } => {
                assert!(matches!(status, ToolStatus::Success));
                let result = result.expect("tool result");
                assert!(matches!(result.r#type, ToolResultValueType::Json));
                assert_eq!(result.value, json!({"path": "out.png"}));
            }
            _ => panic!("expected mcp tool entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_web_search_updates_query() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        push_codex_event(
            &msg_store,
            EventMsg::WebSearchBegin(WebSearchBeginEvent {
                call_id: "web-1".to_string(),
            }),
        );

        push_codex_event(
            &msg_store,
            EventMsg::WebSearchEnd(WebSearchEndEvent {
                call_id: "web-1".to_string(),
                query: "rust lang".to_string(),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "web_search" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::WebFetch { url },
                status,
                ..
            } => {
                assert_eq!(url, "rust lang");
                assert!(matches!(status, ToolStatus::Success));
                assert_eq!(entry.content, "rust lang");
            }
            _ => panic!("expected web search entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_view_image_makes_relative_path() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        push_codex_event(
            &msg_store,
            EventMsg::ViewImageToolCall(ViewImageToolCallEvent {
                call_id: "img-1".to_string(),
                path: PathBuf::from("/repo/images/pic.png"),
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "view_image" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::FileRead { path },
                status,
                ..
            } => {
                assert_eq!(path, "images/pic.png");
                assert!(matches!(status, ToolStatus::Success));
                assert_eq!(entry.content, "images/pic.png");
            }
            _ => panic!("expected view image entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_plan_update_builds_todos() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        push_codex_event(
            &msg_store,
            EventMsg::PlanUpdate(UpdatePlanArgs {
                explanation: None,
                plan: vec![
                    PlanItemArg {
                        step: "First".to_string(),
                        status: StepStatus::Pending,
                    },
                    PlanItemArg {
                        step: "Second".to_string(),
                        status: StepStatus::Completed,
                    },
                ],
            }),
        );

        let entry = wait_for_entry(&msg_store, |entry| match &entry.entry_type {
            NormalizedEntryType::ToolUse {
                tool_name, status, ..
            } => tool_name == "plan" && matches!(status, ToolStatus::Success),
            _ => false,
        })
        .await;

        match entry.entry_type {
            NormalizedEntryType::ToolUse {
                action_type: ActionType::TodoManagement { todos, operation },
                status,
                ..
            } => {
                assert!(matches!(status, ToolStatus::Success));
                assert_eq!(operation, "update");
                assert_eq!(todos.len(), 2);
                assert_eq!(todos[0].content, "First");
                assert_eq!(todos[0].status, "pending");
                assert_eq!(todos[1].content, "Second");
                assert_eq!(todos[1].status, "completed");
                assert_eq!(entry.content, "Plan updated (2 steps)");
            }
            _ => panic!("expected plan entry"),
        }

        msg_store.push_finished();
    }

    #[tokio::test]
    async fn normalize_logs_handles_new_conversation_response() {
        let msg_store = Arc::new(MsgStore::new());
        normalize_logs(msg_store.clone(), std::path::Path::new("/repo"));

        let session_id = "123e4567-e89b-12d3-a456-426614174000";
        let response = JSONRPCResponse {
            id: RequestId::String("1".to_string()),
            result: serde_json::to_value(NewConversationResponse {
                conversation_id: ThreadId::new(),
                model: "gpt-4.1".to_string(),
                reasoning_effort: Some(ReasoningEffort::High),
                rollout_path: PathBuf::from(format!(
                    "/tmp/rollout-2024-01-01T00-00-00-{session_id}.jsonl"
                )),
            })
            .expect("response"),
        };

        push_json_line(
            &msg_store,
            serde_json::to_string(&response).expect("response line"),
        );

        let entry = wait_for_entry(&msg_store, |entry| {
            matches!(entry.entry_type, NormalizedEntryType::SystemMessage)
                && entry.content.contains("model: gpt-4.1")
        })
        .await;

        assert!(entry.content.contains("model: gpt-4.1"));
        assert!(
            msg_store
                .get_history()
                .iter()
                .any(|msg| matches!(msg, LogMsg::SessionId(id) if id == session_id))
        );

        msg_store.push_finished();
    }
}
