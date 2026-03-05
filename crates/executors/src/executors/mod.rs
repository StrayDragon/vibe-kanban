use std::{path::Path, sync::Arc};

use async_trait::async_trait;
pub use executors_core::executors::{
    AppendPrompt, AvailabilityInfo, BaseAgentCapability, ExecutorError, ExecutorExitResult,
    ExecutorExitSignal, InterruptSender, SpawnedChild, StandardCodingAgentExecutor, acp,
};
use executors_core::{
    approvals::ExecutorApprovalService, auto_retry::AutoRetryConfig, env::ExecutionEnv,
};
use executors_protocol::{BaseCodingAgent, actions::ExecutorAction};
use logs_store::MsgStore;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use ts_rs::TS;

use crate::mcp_config::{Adapter, McpConfig, preconfigured_mcp};

#[cfg(feature = "amp")]
pub mod amp {
    pub use executor_amp::amp::*;
}

#[cfg(feature = "claude")]
pub mod claude {
    pub use executor_claude::claude::*;
}

#[cfg(feature = "codex")]
pub mod codex {
    pub use executor_codex::codex::*;
}

#[cfg(feature = "copilot")]
pub mod copilot {
    pub use executor_copilot::copilot::*;
}

#[cfg(feature = "cursor")]
pub mod cursor {
    pub use executor_cursor::cursor::*;
}

#[cfg(feature = "droid")]
pub mod droid {
    pub use executor_droid::droid::*;
}

#[cfg(feature = "fake-agent")]
pub mod fake_agent {
    pub use executor_fake_agent::fake_agent::*;
}

#[cfg(feature = "gemini")]
pub mod gemini {
    pub use executor_gemini::gemini::*;
}

#[cfg(feature = "opencode")]
pub mod opencode {
    pub use executor_opencode::opencode::*;
}

#[cfg(feature = "qwen")]
pub mod qwen {
    pub use executor_qwen::qwen::*;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum CodingAgent {
    #[cfg(feature = "claude")]
    ClaudeCode(claude::ClaudeCode),
    #[cfg(feature = "amp")]
    Amp(amp::Amp),
    #[cfg(feature = "gemini")]
    Gemini(gemini::Gemini),
    #[cfg(feature = "codex")]
    Codex(codex::Codex),
    #[cfg(feature = "fake-agent")]
    FakeAgent(fake_agent::FakeAgent),
    #[cfg(feature = "opencode")]
    Opencode(opencode::Opencode),
    #[cfg(feature = "cursor")]
    CursorAgent(cursor::CursorAgent),
    #[cfg(feature = "qwen")]
    QwenCode(qwen::QwenCode),
    #[cfg(feature = "copilot")]
    Copilot(copilot::Copilot),
    #[cfg(feature = "droid")]
    Droid(droid::Droid),
}

impl CodingAgent {
    pub fn base_agent(&self) -> BaseCodingAgent {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(_) => BaseCodingAgent::ClaudeCode,
            #[cfg(feature = "amp")]
            Self::Amp(_) => BaseCodingAgent::Amp,
            #[cfg(feature = "gemini")]
            Self::Gemini(_) => BaseCodingAgent::Gemini,
            #[cfg(feature = "codex")]
            Self::Codex(_) => BaseCodingAgent::Codex,
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(_) => BaseCodingAgent::FakeAgent,
            #[cfg(feature = "opencode")]
            Self::Opencode(_) => BaseCodingAgent::Opencode,
            #[cfg(feature = "cursor")]
            Self::CursorAgent(_) => BaseCodingAgent::CursorAgent,
            #[cfg(feature = "qwen")]
            Self::QwenCode(_) => BaseCodingAgent::QwenCode,
            #[cfg(feature = "copilot")]
            Self::Copilot(_) => BaseCodingAgent::Copilot,
            #[cfg(feature = "droid")]
            Self::Droid(_) => BaseCodingAgent::Droid,
        }
    }

    pub fn auto_retry_config(&self) -> &AutoRetryConfig {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(cfg) => &cfg.auto_retry,
            #[cfg(feature = "amp")]
            Self::Amp(cfg) => &cfg.auto_retry,
            #[cfg(feature = "gemini")]
            Self::Gemini(cfg) => &cfg.auto_retry,
            #[cfg(feature = "codex")]
            Self::Codex(cfg) => &cfg.auto_retry,
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(cfg) => &cfg.auto_retry,
            #[cfg(feature = "opencode")]
            Self::Opencode(cfg) => &cfg.auto_retry,
            #[cfg(feature = "cursor")]
            Self::CursorAgent(cfg) => &cfg.auto_retry,
            #[cfg(feature = "qwen")]
            Self::QwenCode(cfg) => &cfg.auto_retry,
            #[cfg(feature = "copilot")]
            Self::Copilot(cfg) => &cfg.auto_retry,
            #[cfg(feature = "droid")]
            Self::Droid(cfg) => &cfg.auto_retry,
        }
    }

    pub fn validate_auto_retry(&self) -> Result<(), String> {
        self.auto_retry_config().validate()
    }

    fn preconfigured_mcp(&self) -> serde_json::Value {
        let adapter = match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(_) => Adapter::Passthrough,
            #[cfg(feature = "amp")]
            Self::Amp(_) => Adapter::Passthrough,
            #[cfg(feature = "droid")]
            Self::Droid(_) => Adapter::Passthrough,
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(_) => Adapter::Passthrough,
            #[cfg(feature = "qwen")]
            Self::QwenCode(_) => Adapter::Gemini,
            #[cfg(feature = "gemini")]
            Self::Gemini(_) => Adapter::Gemini,
            #[cfg(feature = "cursor")]
            Self::CursorAgent(_) => Adapter::Cursor,
            #[cfg(feature = "codex")]
            Self::Codex(_) => Adapter::Codex,
            #[cfg(feature = "opencode")]
            Self::Opencode(_) => Adapter::Opencode,
            #[cfg(feature = "copilot")]
            Self::Copilot(..) => Adapter::Copilot,
        };

        preconfigured_mcp(adapter)
    }

    pub fn get_mcp_config(&self) -> McpConfig {
        match self {
            #[cfg(feature = "codex")]
            Self::Codex(_) => McpConfig::new(
                vec!["mcp_servers".to_string()],
                serde_json::json!({
                    "mcp_servers": {}
                }),
                self.preconfigured_mcp(),
                true,
            ),
            #[cfg(feature = "amp")]
            Self::Amp(_) => McpConfig::new(
                vec!["amp.mcpServers".to_string()],
                serde_json::json!({
                    "amp.mcpServers": {}
                }),
                self.preconfigured_mcp(),
                false,
            ),
            #[cfg(feature = "opencode")]
            Self::Opencode(_) => McpConfig::new(
                vec!["mcp".to_string()],
                serde_json::json!({
                    "mcp": {},
                    "$schema": "https://opencode.ai/config.json"
                }),
                self.preconfigured_mcp(),
                false,
            ),
            #[cfg(feature = "droid")]
            Self::Droid(_) => McpConfig::new(
                vec!["mcpServers".to_string()],
                serde_json::json!({
                    "mcpServers": {}
                }),
                self.preconfigured_mcp(),
                false,
            ),
            _ => McpConfig::new(
                vec!["mcpServers".to_string()],
                serde_json::json!({
                    "mcpServers": {}
                }),
                self.preconfigured_mcp(),
                false,
            ),
        }
    }

    pub fn supports_mcp(&self) -> bool {
        StandardCodingAgentExecutor::default_mcp_config_path(self).is_some()
    }

    pub fn capabilities(&self) -> Vec<BaseAgentCapability> {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "amp")]
            Self::Amp(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "gemini")]
            Self::Gemini(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "qwen")]
            Self::QwenCode(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "droid")]
            Self::Droid(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "opencode")]
            Self::Opencode(_) => vec![BaseAgentCapability::SessionFork],
            #[cfg(feature = "codex")]
            Self::Codex(_) => vec![
                BaseAgentCapability::SessionFork,
                BaseAgentCapability::SetupHelper,
            ],
            #[cfg(feature = "cursor")]
            Self::CursorAgent(_) => vec![BaseAgentCapability::SetupHelper],
            #[cfg(feature = "copilot")]
            Self::Copilot(_) => vec![],
        }
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for CodingAgent {
    fn use_approvals(&mut self, approvals: Arc<dyn ExecutorApprovalService>) {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "amp")]
            Self::Amp(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "codex")]
            Self::Codex(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => agent.use_approvals(approvals),
            #[cfg(feature = "droid")]
            Self::Droid(agent) => agent.use_approvals(approvals),
        }
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "amp")]
            Self::Amp(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "codex")]
            Self::Codex(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => agent.spawn(current_dir, prompt, env).await,
            #[cfg(feature = "droid")]
            Self::Droid(agent) => agent.spawn(current_dir, prompt, env).await,
        }
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "amp")]
            Self::Amp(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "codex")]
            Self::Codex(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
            #[cfg(feature = "droid")]
            Self::Droid(agent) => {
                agent
                    .spawn_follow_up(current_dir, prompt, session_id, env)
                    .await
            }
        }
    }

    fn normalize_logs(&self, raw_logs_event_store: Arc<MsgStore>, worktree_path: &Path) {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "amp")]
            Self::Amp(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "codex")]
            Self::Codex(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
            #[cfg(feature = "droid")]
            Self::Droid(agent) => agent.normalize_logs(raw_logs_event_store, worktree_path),
        }
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "amp")]
            Self::Amp(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "codex")]
            Self::Codex(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => agent.default_mcp_config_path(),
            #[cfg(feature = "droid")]
            Self::Droid(agent) => agent.default_mcp_config_path(),
        }
    }

    async fn get_setup_helper_action(&self) -> Result<ExecutorAction, ExecutorError> {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "amp")]
            Self::Amp(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "codex")]
            Self::Codex(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => agent.get_setup_helper_action().await,
            #[cfg(feature = "droid")]
            Self::Droid(agent) => agent.get_setup_helper_action().await,
        }
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        match self {
            #[cfg(feature = "claude")]
            Self::ClaudeCode(agent) => agent.get_availability_info(),
            #[cfg(feature = "amp")]
            Self::Amp(agent) => agent.get_availability_info(),
            #[cfg(feature = "gemini")]
            Self::Gemini(agent) => agent.get_availability_info(),
            #[cfg(feature = "codex")]
            Self::Codex(agent) => agent.get_availability_info(),
            #[cfg(feature = "fake-agent")]
            Self::FakeAgent(agent) => agent.get_availability_info(),
            #[cfg(feature = "opencode")]
            Self::Opencode(agent) => agent.get_availability_info(),
            #[cfg(feature = "cursor")]
            Self::CursorAgent(agent) => agent.get_availability_info(),
            #[cfg(feature = "qwen")]
            Self::QwenCode(agent) => agent.get_availability_info(),
            #[cfg(feature = "copilot")]
            Self::Copilot(agent) => agent.get_availability_info(),
            #[cfg(feature = "droid")]
            Self::Droid(agent) => agent.get_availability_info(),
        }
    }
}
