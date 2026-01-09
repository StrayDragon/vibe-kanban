use std::{
    collections::HashMap,
    env, fs,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use codex_app_server_protocol::JSONRPCNotification;
use codex_mcp_types::{CallToolResult, ContentBlock, TextContent};
use codex_protocol::{
    ConversationId,
    protocol::{
        AgentMessageDeltaEvent, AgentMessageEvent, AgentReasoningDeltaEvent, AskForApproval,
        BackgroundEventEvent, EventMsg, ExecApprovalRequestEvent, ExecCommandBeginEvent,
        ExecCommandEndEvent, ExecCommandOutputDeltaEvent, ExecCommandSource, ExecOutputStream,
        FileChange as CodexFileChange, McpInvocation, McpToolCallBeginEvent, McpToolCallEndEvent,
        PatchApplyBeginEvent, PatchApplyEndEvent, SandboxPolicy, StreamErrorEvent, WarningEvent,
        WebSearchBeginEvent, WebSearchEndEvent,
    },
};
use command_group::AsyncCommandGroup;
use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use tokio::process::Command;
use ts_rs::TS;
use uuid::Uuid;
use workspace_utils::{msg_store::MsgStore, shell::resolve_executable_path_blocking};

use crate::{
    auto_retry::AutoRetryConfig,
    command::{CmdOverrides, CommandBuildError, CommandBuilder, CommandParts, apply_overrides},
    env::ExecutionEnv,
    executors::{
        AppendPrompt, AvailabilityInfo, ExecutorError, SpawnedChild, StandardCodingAgentExecutor,
        codex::normalize_logs::{Approval, normalize_logs},
    },
};

const FAKE_AGENT_CONFIG_ENV: &str = "VIBE_FAKE_AGENT_CONFIG";
const FAKE_AGENT_PATH_ENV: &str = "VIBE_FAKE_AGENT_PATH";

fn default_cadence_ms() -> u64 {
    120
}

fn default_chunk_min() -> usize {
    4
}

fn default_chunk_max() -> usize {
    14
}

fn default_true() -> bool {
    true
}

fn default_write_fake_files() -> bool {
    true
}

fn default_include_reasoning() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct FakeToolEvents {
    #[serde(default = "default_true")]
    pub exec_command: bool,
    #[serde(default = "default_true")]
    pub apply_patch: bool,
    #[serde(default = "default_true")]
    pub mcp: bool,
    #[serde(default = "default_true")]
    pub web_search: bool,
    #[serde(default = "default_true")]
    pub approvals: bool,
    #[serde(default = "default_true")]
    pub errors: bool,
}

impl Default for FakeToolEvents {
    fn default() -> Self {
        Self {
            exec_command: true,
            apply_patch: true,
            mcp: true,
            web_search: true,
            approvals: true,
            errors: true,
        }
    }
}

#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct FakeAgent {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default)]
    pub auto_retry: AutoRetryConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default = "default_cadence_ms")]
    pub cadence_ms: u64,
    #[serde(default = "default_chunk_min")]
    pub message_chunk_min: usize,
    #[serde(default = "default_chunk_max")]
    pub message_chunk_max: usize,
    #[serde(default)]
    pub tool_events: FakeToolEvents,
    #[serde(default = "default_write_fake_files")]
    pub write_fake_files: bool,
    #[serde(default = "default_include_reasoning")]
    pub include_reasoning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_path: Option<String>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,
}

impl FakeAgent {
    fn build_command_parts(&self) -> Result<CommandParts, CommandBuildError> {
        if self.cmd.base_command_override.is_some() {
            let builder = apply_overrides(CommandBuilder::new(fake_agent_binary_name()), &self.cmd);
            return builder.build_initial();
        }

        if let Ok(value) = env::var(FAKE_AGENT_PATH_ENV) && !value.trim().is_empty() {
            let builder = apply_overrides(CommandBuilder::new(value), &self.cmd);
            return builder.build_initial();
        }

        let program = resolve_fake_agent_program();
        let mut args = Vec::new();
        if let Some(extra) = &self.cmd.additional_params {
            args.extend(extra.clone());
        }
        Ok(CommandParts::new(program, args))
    }

    fn runtime_config(&self, prompt: String, session_id: Option<String>) -> FakeAgentRuntimeConfig {
        FakeAgentRuntimeConfig {
            prompt,
            session_id,
            seed: self.seed,
            cadence_ms: self.cadence_ms,
            message_chunk_min: self.message_chunk_min,
            message_chunk_max: self.message_chunk_max,
            tool_events: self.tool_events.clone(),
            write_fake_files: self.write_fake_files,
            include_reasoning: self.include_reasoning,
            scenario_path: self.scenario_path.clone(),
        }
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for FakeAgent {
    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_parts()?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let runtime = self.runtime_config(combined_prompt, None);
        spawn_fake_agent(current_dir, runtime, command_parts, env, &self.cmd).await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_parts()?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let runtime = self.runtime_config(combined_prompt, Some(session_id.to_string()));
        spawn_fake_agent(current_dir, runtime, command_parts, env, &self.cmd).await
    }

    fn normalize_logs(&self, msg_store: Arc<MsgStore>, worktree_path: &Path) {
        normalize_logs(msg_store, worktree_path);
    }

    fn default_mcp_config_path(&self) -> Option<PathBuf> {
        None
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        if fake_agent_available() {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }
}

async fn spawn_fake_agent(
    current_dir: &Path,
    runtime: FakeAgentRuntimeConfig,
    command_parts: CommandParts,
    env: &ExecutionEnv,
    cmd_overrides: &CmdOverrides,
) -> Result<SpawnedChild, ExecutorError> {
    let (program_path, args) = command_parts.into_resolved().await?;
    let config_json = serde_json::to_string(&runtime)?;

    let mut command = Command::new(program_path);
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(current_dir)
        .args(args);

    env.clone()
        .with_profile(cmd_overrides)
        .apply_to_command(&mut command);
    command.env(FAKE_AGENT_CONFIG_ENV, config_json);

    let child = command.group_spawn()?;
    Ok(child.into())
}

fn fake_agent_binary_name() -> String {
    format!("fake-agent{}", std::env::consts::EXE_SUFFIX)
}

fn resolve_fake_agent_program() -> String {
    if let Some(path) = find_sibling_fake_agent() {
        return path.to_string_lossy().to_string();
    }

    fake_agent_binary_name()
}

fn find_sibling_fake_agent() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join(fake_agent_binary_name());
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

fn fake_agent_available() -> bool {
    if let Ok(value) = env::var(FAKE_AGENT_PATH_ENV) {
        return !value.trim().is_empty();
    }
    if find_sibling_fake_agent().is_some() {
        return true;
    }
    resolve_executable_path_blocking(&fake_agent_binary_name()).is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FakeAgentRuntimeConfig {
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(default = "default_cadence_ms")]
    pub cadence_ms: u64,
    #[serde(default = "default_chunk_min")]
    pub message_chunk_min: usize,
    #[serde(default = "default_chunk_max")]
    pub message_chunk_max: usize,
    #[serde(default)]
    pub tool_events: FakeToolEvents,
    #[serde(default = "default_write_fake_files")]
    pub write_fake_files: bool,
    #[serde(default = "default_include_reasoning")]
    pub include_reasoning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_path: Option<String>,
}

#[derive(Debug, Error)]
pub enum FakeAgentError {
    #[error("missing {FAKE_AGENT_CONFIG_ENV} env var")]
    MissingConfig,
    #[error("invalid fake agent config: {0}")]
    InvalidConfig(#[from] serde_json::Error),
    #[error("invalid session id: {0}")]
    InvalidSessionId(#[from] uuid::Error),
    #[error("script parse error: {0}")]
    Script(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone)]
pub enum FakeAgentStep {
    Emit(String),
    Sleep(Duration),
    WriteFile { path: PathBuf, content: String },
}

pub fn run_fake_agent() -> Result<(), FakeAgentError> {
    let raw = env::var(FAKE_AGENT_CONFIG_ENV).map_err(|_| FakeAgentError::MissingConfig)?;
    let mut config: FakeAgentRuntimeConfig = serde_json::from_str(&raw)?;
    normalize_runtime_config(&mut config);
    let cwd = env::current_dir()?;
    let steps = generate_fake_agent_steps(&config, &cwd)?;
    emit_fake_agent_steps(&config, steps)?;
    Ok(())
}

pub fn generate_fake_agent_steps(
    config: &FakeAgentRuntimeConfig,
    cwd: &Path,
) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    if let Some(path) = &config.scenario_path {
        let mut steps = load_script_steps(path, config, cwd)?;
        if !script_has_session_configured(&steps) {
            let session_event = build_session_configured_event(config, cwd)?;
            let line = event_line(session_event)?;
            steps.insert(0, FakeAgentStep::Emit(line));
        }
        return Ok(steps);
    }

    generate_random_steps(config, cwd)
}

fn emit_fake_agent_steps(
    config: &FakeAgentRuntimeConfig,
    steps: Vec<FakeAgentStep>,
) -> Result<(), FakeAgentError> {
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());

    for step in steps {
        match step {
            FakeAgentStep::Emit(line) => {
                writeln!(writer, "{line}")?;
                writer.flush()?;
                if config.cadence_ms > 0 {
                    std::thread::sleep(Duration::from_millis(config.cadence_ms));
                }
            }
            FakeAgentStep::Sleep(duration) => {
                std::thread::sleep(duration);
            }
            FakeAgentStep::WriteFile { path, content } => {
                if config.write_fake_files {
                    write_fake_file(&path, &content)?;
                }
            }
        }
    }

    Ok(())
}

fn normalize_runtime_config(config: &mut FakeAgentRuntimeConfig) {
    if config.message_chunk_min == 0 {
        config.message_chunk_min = default_chunk_min();
    }
    if config.message_chunk_max < config.message_chunk_min {
        config.message_chunk_max = config.message_chunk_min;
    }
}

fn load_script_steps(
    path: &str,
    config: &FakeAgentRuntimeConfig,
    cwd: &Path,
) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut steps = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(trimmed)?;
        if value.get("method").is_some() {
            let raw = serde_json::to_string(&value)?;
            steps.push(FakeAgentStep::Emit(raw));
            continue;
        }
        if let Some(kind) = value.get("type").and_then(|v| v.as_str()) {
            match kind {
                "sleep" | "wait" => {
                    let ms = value
                        .get("ms")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(config.cadence_ms);
                    steps.push(FakeAgentStep::Sleep(Duration::from_millis(ms)));
                    continue;
                }
                "approval_response" => {
                    let approval = parse_approval_step(value)?;
                    steps.push(FakeAgentStep::Emit(approval.raw()));
                    continue;
                }
                _ => {}
            }
        }

        let event: EventMsg = serde_json::from_value(value)?;
        steps.push(FakeAgentStep::Emit(event_line(event)?));
    }

    let mut write_step = None;
    if config.write_fake_files {
        for step in &steps {
            if let FakeAgentStep::Emit(line) = step
                && let Some(path) = extract_fake_patch_path(line, cwd)
            {
                let content = format!(
                    "fake agent wrote {}\n",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
                );
                write_step = Some(FakeAgentStep::WriteFile { path, content });
                break;
            }
        }
    }
    if let Some(step) = write_step {
        steps.push(step);
    }

    Ok(steps)
}

fn parse_approval_step(value: serde_json::Value) -> Result<Approval, FakeAgentError> {
    #[derive(Deserialize)]
    struct ApprovalStep {
        call_id: String,
        tool_name: String,
        #[serde(flatten)]
        status: workspace_utils::approvals::ApprovalStatus,
    }
    let step: ApprovalStep = serde_json::from_value(value)?;
    Ok(Approval::approval_response(
        step.call_id,
        step.tool_name,
        step.status,
    ))
}

fn extract_fake_patch_path(line: &str, cwd: &Path) -> Option<PathBuf> {
    let notification: JSONRPCNotification = serde_json::from_str(line).ok()?;
    let params = notification.params?;
    let msg = params.get("msg")?;
    let event: EventMsg = serde_json::from_value(msg.clone()).ok()?;
    match event {
        EventMsg::PatchApplyBegin(PatchApplyBeginEvent { changes, .. }) => changes
            .keys()
            .next()
            .map(|path| resolve_fake_path(path, cwd)),
        EventMsg::ApplyPatchApprovalRequest(event) => event
            .changes
            .keys()
            .next()
            .map(|path| resolve_fake_path(path, cwd)),
        _ => None,
    }
}

fn resolve_fake_path(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn script_has_session_configured(steps: &[FakeAgentStep]) -> bool {
    for step in steps {
        let FakeAgentStep::Emit(line) = step else {
            continue;
        };
        let Ok(notification) = serde_json::from_str::<JSONRPCNotification>(line) else {
            continue;
        };
        let Some(params) = notification.params else {
            continue;
        };
        let Some(msg) = params.get("msg") else {
            continue;
        };
        let Ok(event) = serde_json::from_value::<EventMsg>(msg.clone()) else {
            continue;
        };
        if matches!(event, EventMsg::SessionConfigured(_)) {
            return true;
        }
    }
    false
}

fn generate_random_steps(
    config: &FakeAgentRuntimeConfig,
    cwd: &Path,
) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    let seed = config.seed.unwrap_or_else(seed_from_time);
    let mut rng = FakeRng::new(seed);
    let session_id = resolve_session_id(config.session_id.as_deref(), &mut rng)?;
    let turn_id = format!("turn-{seed:x}-{:x}", rng.next_u64());

    let mut steps = Vec::new();
    let session_event = build_session_configured_event_with_id(&session_id, cwd)?;
    steps.push(FakeAgentStep::Emit(event_line(session_event)?));

    if config.include_reasoning {
        let reasoning = build_reasoning_text(&mut rng);
        for chunk in chunk_text(
            &reasoning,
            config.message_chunk_min,
            config.message_chunk_max,
            &mut rng,
        ) {
            let event = EventMsg::AgentReasoningDelta(AgentReasoningDeltaEvent { delta: chunk });
            steps.push(FakeAgentStep::Emit(event_line(event)?));
        }
    }

    let tools_summary = build_tools_summary(&config.tool_events);
    if config.tool_events.exec_command {
        steps.extend(generate_exec_command_steps(
            &mut rng,
            cwd,
            &turn_id,
            config.tool_events.approvals,
        )?);
    }
    if config.tool_events.apply_patch {
        steps.extend(generate_patch_steps(
            &mut rng,
            cwd,
            &turn_id,
            config.tool_events.approvals,
            config.write_fake_files,
        )?);
    }
    if config.tool_events.mcp {
        steps.extend(generate_mcp_steps(&mut rng)?);
    }
    if config.tool_events.web_search {
        steps.extend(generate_web_steps(&mut rng)?);
    }
    if config.tool_events.errors {
        let warning = EventMsg::Warning(WarningEvent {
            message: "fake agent warning: simulated issue".to_string(),
        });
        steps.push(FakeAgentStep::Emit(event_line(warning)?));
        let stream_error = EventMsg::StreamError(StreamErrorEvent {
            message: "fake agent stream error: simulated glitch".to_string(),
            codex_error_info: None,
        });
        steps.push(FakeAgentStep::Emit(event_line(stream_error)?));
    }

    let response = build_response_text(&mut rng, &config.prompt, &tools_summary);
    for chunk in chunk_text(
        &response,
        config.message_chunk_min,
        config.message_chunk_max,
        &mut rng,
    ) {
        let event = EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { delta: chunk });
        steps.push(FakeAgentStep::Emit(event_line(event)?));
    }
    let final_event = EventMsg::AgentMessage(AgentMessageEvent { message: response });
    steps.push(FakeAgentStep::Emit(event_line(final_event)?));

    let background = EventMsg::BackgroundEvent(BackgroundEventEvent {
        message: format!("fake agent session {session_id} completed"),
    });
    steps.push(FakeAgentStep::Emit(event_line(background)?));

    Ok(steps)
}

fn build_session_configured_event(
    config: &FakeAgentRuntimeConfig,
    cwd: &Path,
) -> Result<EventMsg, FakeAgentError> {
    let seed = config.seed.unwrap_or_else(seed_from_time);
    let mut rng = FakeRng::new(seed);
    let session_id = resolve_session_id(config.session_id.as_deref(), &mut rng)?;
    build_session_configured_event_with_id(&session_id, cwd)
}

fn build_session_configured_event_with_id(
    session_id: &str,
    cwd: &Path,
) -> Result<EventMsg, FakeAgentError> {
    let session_id = ConversationId::from_string(session_id)?;
    let rollout_path = cwd
        .join(".fake-agent")
        .join(format!("rollout-{session_id}.jsonl"));
    Ok(EventMsg::SessionConfigured(
        codex_protocol::protocol::SessionConfiguredEvent {
            session_id,
            model: "fake-agent".to_string(),
            model_provider_id: "fake-agent".to_string(),
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            cwd: cwd.to_path_buf(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            skill_load_outcome: None,
            rollout_path,
        },
    ))
}

fn generate_exec_command_steps(
    rng: &mut FakeRng,
    cwd: &Path,
    turn_id: &str,
    approvals: bool,
) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    let call_id = format!("cmd-{:x}", rng.next_u64());
    let command = vec!["echo".to_string(), "fake output".to_string()];
    let parsed_cmd = vec![codex_protocol::parse_command::ParsedCommand::Unknown {
        cmd: command.join(" "),
    }];
    let mut steps = Vec::new();

    if approvals {
        let approval = EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id: call_id.clone(),
            turn_id: turn_id.to_string(),
            command: command.clone(),
            cwd: cwd.to_path_buf(),
            reason: Some("fake agent approval".to_string()),
            proposed_execpolicy_amendment: None,
            parsed_cmd: parsed_cmd.clone(),
        });
        steps.push(FakeAgentStep::Emit(event_line(approval)?));
        let approval_line = Approval::approval_response(
            call_id.clone(),
            "codex.exec_command".to_string(),
            workspace_utils::approvals::ApprovalStatus::Approved,
        );
        steps.push(FakeAgentStep::Emit(approval_line.raw()));
    }

    let begin = EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
        call_id: call_id.clone(),
        process_id: None,
        turn_id: turn_id.to_string(),
        command: command.clone(),
        cwd: cwd.to_path_buf(),
        parsed_cmd: parsed_cmd.clone(),
        source: ExecCommandSource::Agent,
        interaction_input: None,
    });
    steps.push(FakeAgentStep::Emit(event_line(begin)?));

    let output = "fake output\n";
    let delta = EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
        call_id: call_id.clone(),
        stream: ExecOutputStream::Stdout,
        chunk: output.as_bytes().to_vec(),
    });
    steps.push(FakeAgentStep::Emit(event_line(delta)?));

    let end = EventMsg::ExecCommandEnd(ExecCommandEndEvent {
        call_id,
        process_id: None,
        turn_id: turn_id.to_string(),
        command,
        cwd: cwd.to_path_buf(),
        parsed_cmd,
        source: ExecCommandSource::Agent,
        interaction_input: None,
        stdout: output.to_string(),
        stderr: String::new(),
        aggregated_output: output.to_string(),
        exit_code: 0,
        duration: Duration::from_millis(120),
        formatted_output: output.trim().to_string(),
    });
    steps.push(FakeAgentStep::Emit(event_line(end)?));

    Ok(steps)
}

fn generate_patch_steps(
    rng: &mut FakeRng,
    cwd: &Path,
    turn_id: &str,
    approvals: bool,
    write_files: bool,
) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    let call_id = format!("patch-{:x}", rng.next_u64());
    let fake_root = format!("__fake_agent__{:x}", rng.next_u64());
    let file_name = format!("change-{:x}.txt", rng.next_u64());
    let file_path = cwd.join(fake_root).join(file_name);
    let content = format!(
        "fake agent edit {}\n",
        file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
    );
    let mut changes = HashMap::new();
    changes.insert(
        file_path.clone(),
        CodexFileChange::Add {
            content: content.clone(),
        },
    );

    let mut steps = Vec::new();

    if approvals {
        let approval = EventMsg::ApplyPatchApprovalRequest(
            codex_protocol::approvals::ApplyPatchApprovalRequestEvent {
                call_id: call_id.clone(),
                turn_id: turn_id.to_string(),
                changes: changes.clone(),
                reason: Some("fake agent patch approval".to_string()),
                grant_root: None,
            },
        );
        steps.push(FakeAgentStep::Emit(event_line(approval)?));
        let approval_line = Approval::approval_response(
            call_id.clone(),
            "codex.apply_patch".to_string(),
            workspace_utils::approvals::ApprovalStatus::Approved,
        );
        steps.push(FakeAgentStep::Emit(approval_line.raw()));
    }

    let begin = EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
        call_id: call_id.clone(),
        turn_id: turn_id.to_string(),
        auto_approved: !approvals,
        changes: changes.clone(),
    });
    steps.push(FakeAgentStep::Emit(event_line(begin)?));

    if write_files {
        steps.push(FakeAgentStep::WriteFile {
            path: file_path.clone(),
            content: content.clone(),
        });
    }

    let end = EventMsg::PatchApplyEnd(PatchApplyEndEvent {
        call_id,
        turn_id: turn_id.to_string(),
        stdout: "applied".to_string(),
        stderr: String::new(),
        success: true,
        changes,
    });
    steps.push(FakeAgentStep::Emit(event_line(end)?));

    Ok(steps)
}

fn generate_mcp_steps(rng: &mut FakeRng) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    let call_id = format!("mcp-{:x}", rng.next_u64());
    let invocation = McpInvocation {
        server: "fake".to_string(),
        tool: "echo".to_string(),
        arguments: Some(json!({ "text": "mcp payload" })),
    };
    let begin = EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
        call_id: call_id.clone(),
        invocation: invocation.clone(),
    });
    let content = ContentBlock::TextContent(TextContent {
        annotations: None,
        text: "fake mcp result".to_string(),
        r#type: "text".to_string(),
    });
    let result = CallToolResult {
        content: vec![content],
        is_error: None,
        structured_content: None,
    };
    let end = EventMsg::McpToolCallEnd(McpToolCallEndEvent {
        call_id,
        invocation,
        duration: Duration::from_millis(90),
        result: Ok(result),
    });
    Ok(vec![
        FakeAgentStep::Emit(event_line(begin)?),
        FakeAgentStep::Emit(event_line(end)?),
    ])
}

fn generate_web_steps(rng: &mut FakeRng) -> Result<Vec<FakeAgentStep>, FakeAgentError> {
    let call_id = format!("web-{:x}", rng.next_u64());
    let begin = EventMsg::WebSearchBegin(WebSearchBeginEvent {
        call_id: call_id.clone(),
    });
    let query = format!("fake query {}", rng.next_u64() % 100);
    let end = EventMsg::WebSearchEnd(WebSearchEndEvent { call_id, query });
    Ok(vec![
        FakeAgentStep::Emit(event_line(begin)?),
        FakeAgentStep::Emit(event_line(end)?),
    ])
}

fn event_line(event: EventMsg) -> Result<String, FakeAgentError> {
    let params = json!({ "msg": event });
    let notification = JSONRPCNotification {
        method: "codex/event".to_string(),
        params: Some(params),
    };
    Ok(serde_json::to_string(&notification)?)
}

fn write_fake_file(path: &Path, content: &str) -> Result<(), io::Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn build_reasoning_text(rng: &mut FakeRng) -> String {
    let phrases = [
        "Planning simulated actions.",
        "Preparing mock tool calls.",
        "Checking state for deterministic playback.",
        "Staging fake edits and command output.",
    ];
    format!(
        "{} {}",
        phrases[rng.pick_index(phrases.len())],
        phrases[rng.pick_index(phrases.len())]
    )
}

fn build_response_text(rng: &mut FakeRng, prompt: &str, tools: &str) -> String {
    let openers = ["Got it.", "Understood.", "All set.", "Working on it."];
    let closers = [
        "Let me know if you want another run.",
        "Ready for the next step.",
        "You can repeat this with a new seed.",
    ];
    let prompt_snippet = truncate_prompt(prompt, 120);
    format!(
        "{} Simulated tools: {}. Prompt: \"{}\". {}",
        openers[rng.pick_index(openers.len())],
        tools,
        prompt_snippet,
        closers[rng.pick_index(closers.len())]
    )
}

fn build_tools_summary(tool_events: &FakeToolEvents) -> String {
    let mut parts = Vec::new();
    if tool_events.exec_command {
        parts.push("bash");
    }
    if tool_events.apply_patch {
        parts.push("apply_patch");
    }
    if tool_events.mcp {
        parts.push("mcp");
    }
    if tool_events.web_search {
        parts.push("web_search");
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn truncate_prompt(prompt: &str, max_len: usize) -> String {
    let trimmed = prompt.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }
    let snippet: String = trimmed.chars().take(max_len).collect();
    format!("{snippet}...")
}

fn chunk_text(text: &str, min: usize, max: usize, rng: &mut FakeRng) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    let min = min.max(1);
    let max = max.max(min);
    while index < chars.len() {
        let span = if max == min {
            min
        } else {
            min + rng.pick_index(max - min + 1)
        };
        let end = (index + span).min(chars.len());
        let chunk: String = chars[index..end].iter().collect();
        chunks.push(chunk);
        index = end;
    }
    chunks
}

fn resolve_session_id(
    session_id: Option<&str>,
    rng: &mut FakeRng,
) -> Result<String, FakeAgentError> {
    if let Some(session_id) = session_id {
        ConversationId::from_string(session_id)?;
        return Ok(session_id.to_string());
    }
    let high = rng.next_u64() as u128;
    let low = rng.next_u64() as u128;
    let uuid = Uuid::from_u128((high << 64) | low);
    Ok(uuid.to_string())
}

fn seed_from_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xfeed_face)
}

#[derive(Debug, Clone)]
struct FakeRng {
    state: u64,
}

impl FakeRng {
    fn new(seed: u64) -> Self {
        let seed = if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        };
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn pick_index(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        (self.next_u64() as usize) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit_lines(steps: &[FakeAgentStep]) -> Vec<String> {
        steps
            .iter()
            .filter_map(|step| match step {
                FakeAgentStep::Emit(line) => Some(line.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn deterministic_output_with_seed() {
        let config = FakeAgentRuntimeConfig {
            prompt: "test prompt".to_string(),
            session_id: None,
            seed: Some(42),
            cadence_ms: 0,
            message_chunk_min: 4,
            message_chunk_max: 8,
            tool_events: FakeToolEvents::default(),
            write_fake_files: false,
            include_reasoning: true,
            scenario_path: None,
        };
        let cwd = std::env::temp_dir();
        let first = generate_fake_agent_steps(&config, &cwd).expect("first");
        let second = generate_fake_agent_steps(&config, &cwd).expect("second");
        assert_eq!(emit_lines(&first), emit_lines(&second));
    }

    #[test]
    fn follow_up_preserves_session_id() {
        let session_id = Uuid::from_u128(7).to_string();
        let config = FakeAgentRuntimeConfig {
            prompt: "follow-up".to_string(),
            session_id: Some(session_id.clone()),
            seed: Some(9),
            cadence_ms: 0,
            message_chunk_min: 4,
            message_chunk_max: 8,
            tool_events: FakeToolEvents::default(),
            write_fake_files: false,
            include_reasoning: false,
            scenario_path: None,
        };
        let cwd = std::env::temp_dir();
        let steps = generate_fake_agent_steps(&config, &cwd).expect("steps");
        let first = emit_lines(&steps).into_iter().next().expect("first line");
        let notification: JSONRPCNotification = serde_json::from_str(&first).expect("notification");
        let params = notification.params.expect("params");
        let msg = params.get("msg").expect("msg");
        let event: EventMsg = serde_json::from_value(msg.clone()).expect("event");
        if let EventMsg::SessionConfigured(payload) = event {
            assert_eq!(payload.session_id.to_string(), session_id);
        } else {
            panic!("expected SessionConfigured event");
        }
    }
}
