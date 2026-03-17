pub mod client;
pub mod compatibility;
pub mod dynamic_tools;
pub mod jsonrpc;
pub mod normalize_logs;
pub mod session;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use codex_app_server_protocol::{
    AskForApproval as ProtocolAskForApproval, SandboxMode as ProtocolSandboxMode, ThreadForkParams,
    ThreadStartParams, UserInput,
};
use command_group::AsyncCommandGroup;
use derivative::Derivative;
use executors_core::{
    agent_command::{AgentCommandKey, agent_command_resolver, command_identity_for_agent},
    approvals::ExecutorApprovalService,
    auto_retry::AutoRetryConfig,
    command::{CmdOverrides, CommandBuilder, CommandParts, apply_overrides},
    env::ExecutionEnv,
    executors::{
        AppendPrompt, AvailabilityInfo, ExecutorError, ExecutorExitResult, SpawnedChild,
        StandardCodingAgentExecutor,
    },
    log_writer::LogWriter,
    stdout_dup::create_stdout_pipe_writer,
};
use executors_protocol::BaseCodingAgent;
use logs_store::MsgStore;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum_macros::AsRefStr;
use tokio::process::Command;
use ts_rs::TS;

use self::{
    client::AppServerClient,
    jsonrpc::{ExitSignalSender, JsonRpcPeer},
    normalize_logs::{Error, normalize_logs},
};

/// Sandbox policy modes for Codex
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SandboxMode {
    Auto,
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// Determines when the user is consulted to approve Codex actions.
///
/// - `UnlessTrusted`: Read-only commands are auto-approved. Everything else will
///   ask the user to approve.
/// - `OnFailure`: All commands run in a restricted sandbox initially. If a
///   command fails, the user is asked to approve execution without the sandbox.
/// - `OnRequest`: The model decides when to ask the user for approval.
/// - `Never`: Commands never ask for approval. Commands that fail in the
///   restricted sandbox are not retried.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum AskForApproval {
    UnlessTrusted,
    OnFailure,
    OnRequest,
    Never,
}

/// Reasoning effort for the underlying model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
    Xhigh,
}

/// Model reasoning summary style
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

/// Format for model reasoning summaries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ReasoningSummaryFormat {
    None,
    Experimental,
}

#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct Codex {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default)]
    pub auto_retry: AutoRetryConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_for_approval: Option<AskForApproval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_summary: Option<ReasoningSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_summary_format: Option<ReasoningSummaryFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_apply_patch_tool: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_dynamic_tools: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,

    #[serde(skip)]
    #[ts(skip)]
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
}

#[async_trait]
impl StandardCodingAgentExecutor for Codex {
    fn use_approvals(&mut self, approvals: Arc<dyn ExecutorApprovalService>) {
        self.approvals = Some(approvals);
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_builder().await.build_initial()?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let input_items = build_input_items(&combined_prompt, None);
        self.spawn_inner(current_dir, input_items, command_parts, None, env)
            .await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_builder().await.build_follow_up(&[])?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let input_items = build_input_items(&combined_prompt, None);
        self.spawn_inner(
            current_dir,
            input_items,
            command_parts,
            Some(session_id),
            env,
        )
        .await
    }

    fn normalize_logs(&self, msg_store: Arc<MsgStore>, worktree_path: &Path) {
        normalize_logs(msg_store, worktree_path);
    }

    fn default_mcp_config_path(&self) -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".codex").join("config.toml"))
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        if let Some(timestamp) = dirs::home_dir()
            .and_then(|home| std::fs::metadata(home.join(".codex").join("auth.json")).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
        {
            return AvailabilityInfo::LoginDetected {
                last_auth_timestamp: timestamp,
            };
        }

        let mcp_config_found = self
            .default_mcp_config_path()
            .map(|p| p.exists())
            .unwrap_or(false);

        let installation_indicator_found = dirs::home_dir()
            .map(|home| home.join(".codex").join("version.json").exists())
            .unwrap_or(false);

        if mcp_config_found || installation_indicator_found {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }
}

impl Codex {
    pub async fn spawn_with_image_paths(
        &self,
        current_dir: &Path,
        prompt: &str,
        image_paths: Option<&HashMap<String, PathBuf>>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_builder().await.build_initial()?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let input_items = build_input_items(&combined_prompt, image_paths);
        self.spawn_inner(current_dir, input_items, command_parts, None, env)
            .await
    }

    pub async fn spawn_follow_up_with_image_paths(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        image_paths: Option<&HashMap<String, PathBuf>>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_builder().await.build_follow_up(&[])?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let input_items = build_input_items(&combined_prompt, image_paths);
        self.spawn_inner(
            current_dir,
            input_items,
            command_parts,
            Some(session_id),
            env,
        )
        .await
    }

    async fn build_command_builder(&self) -> CommandBuilder {
        let resolved = agent_command_resolver()
            .resolve_with_overrides(
                AgentCommandKey::Agent(BaseCodingAgent::Codex),
                command_identity_for_agent(BaseCodingAgent::Codex),
                &self.cmd,
            )
            .await;
        let mut builder = CommandBuilder::new(resolved.base_command);
        if self.oss.unwrap_or(false) {
            // `--oss` is a top-level Codex flag and must come before subcommands.
            builder = builder.extend_params(["--oss"]);
        }
        builder = builder.extend_params(["app-server"]);

        apply_overrides(builder, &self.cmd)
    }

    fn build_thread_start_params(&self, cwd: &Path) -> ThreadStartParams {
        let sandbox = match self.sandbox.as_ref() {
            None | Some(SandboxMode::Auto) => Some(ProtocolSandboxMode::WorkspaceWrite), // match the Auto preset in codex
            Some(SandboxMode::ReadOnly) => Some(ProtocolSandboxMode::ReadOnly),
            Some(SandboxMode::WorkspaceWrite) => Some(ProtocolSandboxMode::WorkspaceWrite),
            Some(SandboxMode::DangerFullAccess) => Some(ProtocolSandboxMode::DangerFullAccess),
        };

        let approval_policy = match self.ask_for_approval.as_ref() {
            None if matches!(self.sandbox.as_ref(), None | Some(SandboxMode::Auto)) => {
                // match the Auto preset in codex
                Some(ProtocolAskForApproval::OnRequest)
            }
            None => None,
            Some(AskForApproval::UnlessTrusted) => Some(ProtocolAskForApproval::UnlessTrusted),
            Some(AskForApproval::OnFailure) => Some(ProtocolAskForApproval::OnFailure),
            Some(AskForApproval::OnRequest) => Some(ProtocolAskForApproval::OnRequest),
            Some(AskForApproval::Never) => Some(ProtocolAskForApproval::Never),
        };

        let dynamic_tools = if self.enable_dynamic_tools.unwrap_or(true) {
            Some(dynamic_tools::VkDynamicToolRegistry::vk_default().specs())
        } else {
            None
        };

        ThreadStartParams {
            model: self.model.clone(),
            model_provider: self.model_provider.clone(),
            service_tier: None,
            cwd: Some(cwd.to_string_lossy().to_string()),
            approval_policy,
            approvals_reviewer: None,
            sandbox,
            config: self.build_config_overrides(),
            service_name: None,
            base_instructions: self.base_instructions.clone(),
            developer_instructions: self.developer_instructions.clone(),
            personality: None,
            ephemeral: None,
            dynamic_tools,
            mock_experimental_field: None,
            experimental_raw_events: false,
            persist_extended_history: false,
        }
    }

    fn build_config_overrides(&self) -> Option<HashMap<String, Value>> {
        let mut overrides = HashMap::new();

        if let Some(effort) = &self.model_reasoning_effort {
            overrides.insert(
                "model_reasoning_effort".to_string(),
                Value::String(effort.as_ref().to_string()),
            );
        }

        if let Some(summary) = &self.model_reasoning_summary {
            overrides.insert(
                "model_reasoning_summary".to_string(),
                Value::String(summary.as_ref().to_string()),
            );
        }

        if let Some(format) = &self.model_reasoning_summary_format
            && format != &ReasoningSummaryFormat::None
        {
            overrides.insert(
                "model_reasoning_summary_format".to_string(),
                Value::String(format.as_ref().to_string()),
            );
        }

        if overrides.is_empty() {
            None
        } else {
            Some(overrides)
        }
    }

    async fn spawn_inner(
        &self,
        current_dir: &Path,
        input_items: Vec<UserInput>,
        command_parts: CommandParts,
        resume_session: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let compat = compatibility::check_codex_protocol_compatibility(
            &self.cmd,
            self.oss.unwrap_or(false),
            false,
        )
        .await;
        if compat.status == compatibility::CodexProtocolCompatibilityStatus::Compatible
            && compat.message.is_some()
        {
            tracing::warn!(
                expected_v2_schema_sha256 = %compat.expected_v2_schema_sha256,
                runtime_v2_schema_sha256 = ?compat.runtime_v2_schema_sha256,
                codex_cli_version = ?compat.codex_cli_version,
                base_command = %compat.base_command,
                message = ?compat.message,
                "proceeding with Codex despite protocol fingerprint drift"
            );
        }
        if matches!(
            compat.status,
            compatibility::CodexProtocolCompatibilityStatus::Incompatible
                | compatibility::CodexProtocolCompatibilityStatus::Unknown
        ) {
            tracing::error!(
                status = ?compat.status,
                expected_v2_schema_sha256 = %compat.expected_v2_schema_sha256,
                runtime_v2_schema_sha256 = ?compat.runtime_v2_schema_sha256,
                codex_cli_version = ?compat.codex_cli_version,
                base_command = %compat.base_command,
                message = ?compat.message,
                "blocking Codex spawn due to protocol compatibility"
            );
            return Err(ExecutorError::Io(std::io::Error::other(
                compatibility::compatibility_blocking_error_message(&compat),
            )));
        }

        let (program_path, args) = command_parts.into_resolved().await?;

        let mut process = Command::new(program_path);
        process
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(current_dir)
            .args(&args)
            .env("NODE_NO_WARNINGS", "1")
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "error");

        env.clone()
            .with_profile(&self.cmd)
            .apply_to_command(&mut process);

        let mut child = process.group_spawn()?;

        let child_stdout = child.inner().stdout.take().ok_or_else(|| {
            ExecutorError::Io(std::io::Error::other("Codex app server missing stdout"))
        })?;
        let child_stdin = child.inner().stdin.take().ok_or_else(|| {
            ExecutorError::Io(std::io::Error::other("Codex app server missing stdin"))
        })?;

        let new_stdout = create_stdout_pipe_writer(&mut child)?;
        let (exit_signal_tx, exit_signal_rx) = tokio::sync::oneshot::channel();

        let params = self.build_thread_start_params(current_dir);
        let resume_session = resume_session.map(|s| s.to_string());
        let auto_approve = matches!(
            (&self.sandbox, &self.ask_for_approval),
            (Some(SandboxMode::DangerFullAccess), None)
        );
        let approvals = self.approvals.clone();
        let mut dynamic_tool_context =
            dynamic_tools::VkDynamicToolContext::new(current_dir.to_path_buf());
        dynamic_tool_context.project_name = env.vars.get("VK_PROJECT_NAME").cloned();
        dynamic_tool_context.project_id = env.vars.get("VK_PROJECT_ID").cloned();
        dynamic_tool_context.task_id = env.vars.get("VK_TASK_ID").cloned();
        dynamic_tool_context.attempt_id = env.vars.get("VK_WORKSPACE_ID").cloned();
        dynamic_tool_context.workspace_branch = env.vars.get("VK_WORKSPACE_BRANCH").cloned();

        tokio::spawn(async move {
            let exit_signal_tx = ExitSignalSender::new(exit_signal_tx);
            let log_writer = LogWriter::new(new_stdout);
            if let Err(err) = Self::launch_codex_app_server(
                params,
                resume_session,
                input_items,
                child_stdout,
                child_stdin,
                log_writer.clone(),
                exit_signal_tx.clone(),
                approvals,
                auto_approve,
                dynamic_tool_context,
            )
            .await
            {
                match &err {
                    ExecutorError::Io(io_err)
                        if io_err.kind() == std::io::ErrorKind::BrokenPipe =>
                    {
                        // Broken pipe likely means the parent process exited, so we can ignore it
                        return;
                    }
                    ExecutorError::AuthRequired(message) => {
                        log_writer
                            .log_raw(&Error::auth_required(message.clone()).raw())
                            .await
                            .ok();
                        // Send failure signal so the process is marked as failed
                        exit_signal_tx
                            .send_exit_signal(ExecutorExitResult::Failure)
                            .await;
                        return;
                    }
                    _ => {
                        tracing::error!("Codex spawn error: {}", err);
                        log_writer
                            .log_raw(&Error::launch_error(err.to_string()).raw())
                            .await
                            .ok();
                    }
                }
                // For other errors, also send failure signal
                exit_signal_tx
                    .send_exit_signal(ExecutorExitResult::Failure)
                    .await;
            }
        });

        Ok(SpawnedChild {
            child,
            exit_signal: Some(exit_signal_rx),
            interrupt_sender: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn launch_codex_app_server(
        conversation_params: ThreadStartParams,
        resume_session: Option<String>,
        input_items: Vec<UserInput>,
        child_stdout: tokio::process::ChildStdout,
        child_stdin: tokio::process::ChildStdin,
        log_writer: LogWriter,
        exit_signal_tx: ExitSignalSender,
        approvals: Option<Arc<dyn ExecutorApprovalService>>,
        auto_approve: bool,
        dynamic_tool_context: dynamic_tools::VkDynamicToolContext,
    ) -> Result<(), ExecutorError> {
        let client =
            AppServerClient::new(log_writer, approvals, auto_approve, dynamic_tool_context);
        let rpc_peer =
            JsonRpcPeer::spawn(child_stdin, child_stdout, client.clone(), exit_signal_tx);
        client.connect(rpc_peer);
        client.initialize().await?;
        let auth_status = client.get_auth_status().await?;
        if auth_status.requires_openai_auth.unwrap_or(true) && auth_status.auth_method.is_none() {
            return Err(ExecutorError::AuthRequired(
                "Codex authentication required".to_string(),
            ));
        }
        match resume_session {
            None => {
                let params = conversation_params;
                let thread_id = client.thread_start(params).await?;
                client.register_thread(&thread_id).await?;
                client.turn_start(&thread_id, input_items).await?;
            }
            Some(session_id) => {
                let overrides = conversation_params;
                let thread_id = client
                    .thread_fork(ThreadForkParams {
                        thread_id: session_id.clone(),
                        path: None,
                        model: overrides.model,
                        model_provider: overrides.model_provider,
                        service_tier: overrides.service_tier,
                        cwd: overrides.cwd,
                        approval_policy: overrides.approval_policy,
                        approvals_reviewer: overrides.approvals_reviewer,
                        sandbox: overrides.sandbox,
                        config: overrides.config,
                        base_instructions: overrides.base_instructions,
                        developer_instructions: overrides.developer_instructions,
                        ephemeral: overrides.ephemeral.unwrap_or(false),
                        persist_extended_history: overrides.persist_extended_history,
                    })
                    .await?;
                tracing::debug!("forked thread for session {session_id}: {thread_id}");
                client.register_thread(&thread_id).await?;
                client.turn_start(&thread_id, input_items).await?;
            }
        }
        Ok(())
    }
}

fn build_input_items(
    prompt: &str,
    image_paths: Option<&HashMap<String, PathBuf>>,
) -> Vec<UserInput> {
    let Some(image_paths) = image_paths else {
        return vec![UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        }];
    };

    let pattern =
        Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").unwrap_or_else(|_| Regex::new("$^").unwrap());
    let mut items = Vec::new();
    let mut last = 0;

    let push_text = |items: &mut Vec<UserInput>, text: &str| {
        if text.is_empty() {
            return;
        }
        if let Some(UserInput::Text { text: existing, .. }) = items.last_mut() {
            existing.push_str(text);
            return;
        }
        items.push(UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        });
    };

    for caps in pattern.captures_iter(prompt) {
        let Some(full_match) = caps.get(0) else {
            continue;
        };
        let src = caps.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
        let end = full_match.end();
        if last < end {
            push_text(&mut items, &prompt[last..end]);
        }
        if let Some(path) = image_paths.get(src).filter(|path| path.exists()) {
            items.push(UserInput::LocalImage { path: path.clone() });
        }
        last = end;
    }

    if last < prompt.len() {
        push_text(&mut items, &prompt[last..]);
    }

    if items.is_empty() {
        items.push(UserInput::Text {
            text: prompt.to_string(),
            text_elements: Vec::new(),
        });
    }

    items
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use codex_app_server_protocol::UserInput;
    use executors_core::{env::ExecutionEnv, executors::StandardCodingAgentExecutor};
    use serde_json::json;

    use super::{Codex, build_input_items, dynamic_tools};

    #[test]
    fn build_input_items_interleaves_images_in_order() {
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        let temp_path = temp.path().to_path_buf();
        let mut map = HashMap::new();
        map.insert(".vibe-images/a.png".to_string(), temp_path.clone());

        let prompt = "Intro ![a](.vibe-images/a.png) end";
        let items = build_input_items(prompt, Some(&map));

        assert_eq!(items.len(), 3);
        match &items[0] {
            UserInput::Text { text, .. } => {
                assert!(text.contains("![a](.vibe-images/a.png)"));
                assert!(text.contains("Intro"));
            }
            _ => panic!("expected text item"),
        }
        match &items[1] {
            UserInput::LocalImage { path } => {
                assert_eq!(path, &temp_path);
            }
            _ => panic!("expected local image item"),
        }
        match &items[2] {
            UserInput::Text { text, .. } => {
                assert!(text.contains(" end"));
            }
            _ => panic!("expected trailing text item"),
        }
    }

    #[test]
    fn build_input_items_skips_missing_images() {
        let mut map = HashMap::<String, PathBuf>::new();
        map.insert(
            ".vibe-images/missing.png".to_string(),
            PathBuf::from("/nonexistent/missing.png"),
        );

        let prompt = "Text ![x](.vibe-images/missing.png) tail";
        let items = build_input_items(prompt, Some(&map));

        assert_eq!(items.len(), 1);
        match &items[0] {
            UserInput::Text { text, .. } => {
                assert!(text.contains("![x](.vibe-images/missing.png)"));
            }
            _ => panic!("expected text item"),
        }
    }

    #[test]
    fn thread_start_params_registers_dynamic_tools_by_default() {
        let executor: Codex = serde_json::from_value(json!({})).expect("default config");
        let params = executor.build_thread_start_params(std::path::Path::new("/tmp"));
        let tools = params.dynamic_tools.expect("dynamic tools registered");
        let names = tools.into_iter().map(|t| t.name).collect::<Vec<_>>();
        assert!(
            names
                .iter()
                .any(|n| n == dynamic_tools::VK_TOOL_GET_ATTEMPT_STATUS)
        );
        assert!(
            names
                .iter()
                .any(|n| n == dynamic_tools::VK_TOOL_TAIL_ATTEMPT_LOGS)
        );
        assert!(
            names
                .iter()
                .any(|n| n == dynamic_tools::VK_TOOL_GET_ATTEMPT_CHANGES)
        );
    }

    #[test]
    fn thread_start_params_skips_dynamic_tools_when_disabled() {
        let mut executor: Codex = serde_json::from_value(json!({})).expect("default config");
        executor.enable_dynamic_tools = Some(false);
        let params = executor.build_thread_start_params(std::path::Path::new("/tmp"));
        assert!(params.dynamic_tools.is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_fails_fast_with_compatibility_message_when_incompatible() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let fake_codex = dir.path().join("codex");

        std::fs::write(
            &fake_codex,
            r#"#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  echo "codex-cli 0.0.0-test"
  exit 0
fi

if [ "${1:-}" = "--oss" ]; then
  shift
fi

if [ "${1:-}" = "app-server" ] && [ "${2:-}" = "generate-json-schema" ]; then
  out=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--out" ]; then
      out="$2"
      shift 2
      continue
    fi
    shift
  done
  if [ -z "$out" ]; then
    echo "missing --out" >&2
    exit 1
  fi
  mkdir -p "$out"
  echo '{"vk":"mismatch"}' > "$out/codex_app_server_protocol.v2.schemas.json"
  exit 0
fi

echo "unexpected args: $*" >&2
exit 1
"#,
        )
        .expect("write fake codex");

        let mut perms = std::fs::metadata(&fake_codex)
            .expect("metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_codex, perms).expect("chmod");

        let executor: Codex = serde_json::from_value(json!({
            "base_command_override": fake_codex.to_string_lossy(),
        }))
        .expect("executor config");

        let env = ExecutionEnv::new();
        let err = executor
            .spawn(dir.path(), "hello", &env)
            .await
            .expect_err("spawn should be blocked");

        let msg = err.to_string();
        assert!(msg.contains("Codex protocol is incompatible"));
        assert!(msg.contains("Expected protocol fingerprint"));
        assert!(msg.contains("Fix: upgrade Vibe Kanban"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn dynamic_tool_call_smoke_test_executes_tool() {
        use std::os::unix::fs::PermissionsExt;

        use executors_core::executors::ExecutorExitResult;
        use tokio::time::{Duration, timeout};

        let root = tempfile::tempdir().expect("tempdir");
        let schema_dir = root.path().join("schema");
        std::fs::create_dir_all(&schema_dir).expect("schema dir");
        codex_app_server_protocol::generate_json(&schema_dir)
            .expect("generate pinned protocol schema");

        let bundle_path = schema_dir.join("codex_app_server_protocol.v2.schemas.json");
        assert!(bundle_path.exists(), "schema bundle missing");

        let attempt_id = "3b3a4b8a-3b55-4f8b-9af6-0fef7f7c0b4b";
        let fake_codex = root.path().join("codex");

        let script = r#"#!/bin/sh
set -eu

BUNDLE_PATH="__BUNDLE_PATH__"
ATTEMPT_ID_EXPECT="__ATTEMPT_ID_EXPECT__"

if [ "${1:-}" = "--version" ]; then
  echo "codex-cli 0.0.0-test"
  exit 0
fi

if [ "${1:-}" = "--oss" ]; then
  shift
fi

if [ "${1:-}" = "app-server" ] && [ "${2:-}" = "generate-json-schema" ]; then
  out=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--out" ]; then
      out="$2"
      shift 2
      continue
    fi
    shift
  done
  mkdir -p "$out"
  cp "$BUNDLE_PATH" "$out/codex_app_server_protocol.v2.schemas.json"
  exit 0
fi

if [ "${1:-}" = "app-server" ]; then
  thread_id="thread_123"
  turn_id="turn_1"
  tool_req_id=99
  tool_call_id="call_1"

  while IFS= read -r line; do
    [ -z "$line" ] && continue

    case "$line" in
      *\"method\":\"initialize\"*)
        echo "{\"id\":1,\"result\":{}}"
        ;;
      *\"method\":\"getAuthStatus\"*)
        echo "{\"id\":2,\"result\":{\"authMethod\":\"apikey\",\"requiresOpenaiAuth\":false}}"
        ;;
      *\"method\":\"thread/start\"*)
        echo "$line" | grep -q '\"dynamicTools\"' || { echo "missing dynamicTools" >&2; exit 1; }
        echo "$line" | grep -q 'vk_get_attempt_status' || { echo "missing vk_get_attempt_status" >&2; exit 1; }
        echo "{\"id\":3,\"result\":{\"thread\":{\"id\":\"$thread_id\"}}}"
        ;;
      *\"method\":\"turn/start\"*)
        echo "{\"id\":4,\"result\":{\"turn\":{\"id\":\"$turn_id\",\"items\":[],\"status\":\"completed\",\"error\":null}}}"
        echo "{\"id\":$tool_req_id,\"method\":\"item/tool/call\",\"params\":{\"threadId\":\"$thread_id\",\"turnId\":\"$turn_id\",\"callId\":\"$tool_call_id\",\"tool\":\"vk_get_attempt_status\",\"arguments\":{}}}"
        ;;
      *\"id\":99*)
        echo "$line" | grep -q '\"success\":true' || { echo "tool call not successful" >&2; exit 1; }
        echo "$line" | grep -q "$ATTEMPT_ID_EXPECT" || { echo "missing attempt id in tool output" >&2; exit 1; }
        echo "{\"method\":\"codex/event/turn_complete\"}"
        exit 0
        ;;
      *)
        # ignore
        ;;
    esac
  done

  echo "unexpected EOF" >&2
  exit 1
fi

echo "unexpected args: $*" >&2
exit 1
"#
        .replace(
            "__BUNDLE_PATH__",
            bundle_path.to_string_lossy().as_ref(),
        )
        .replace("__ATTEMPT_ID_EXPECT__", attempt_id);

        std::fs::write(&fake_codex, script).expect("write fake codex");

        let mut perms = std::fs::metadata(&fake_codex)
            .expect("metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&fake_codex, perms).expect("chmod");

        let executor: Codex = serde_json::from_value(json!({
            "base_command_override": fake_codex.to_string_lossy(),
        }))
        .expect("executor config");

        let mut env = ExecutionEnv::new();
        env.insert("VK_WORKSPACE_ID", attempt_id);

        let mut spawned = executor
            .spawn(root.path(), "hello", &env)
            .await
            .expect("spawn");

        let exit_signal = spawned.exit_signal.take().expect("exit signal");
        let result = timeout(Duration::from_secs(5), exit_signal)
            .await
            .expect("timeout")
            .expect("exit result");

        assert!(matches!(result, ExecutorExitResult::Success));
    }
}
