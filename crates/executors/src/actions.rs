use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncCommandGroup;
use executors_core::{approvals::ExecutorApprovalService, env::ExecutionEnv};
use executors_protocol::actions::{
    ExecutorAction, ExecutorActionType,
    coding_agent_follow_up::CodingAgentFollowUpRequest,
    coding_agent_initial::CodingAgentInitialRequest,
    script::{ScriptContext, ScriptRequest},
};
use tokio::process::Command;
use utils_core::shell::get_shell_command;

use crate::{
    executors::{CodingAgent, ExecutorError, SpawnedChild, StandardCodingAgentExecutor},
    profile::ExecutorConfigs,
};

fn resolve_coding_agent(
    executor_profile_id: &executors_protocol::ExecutorProfileId,
) -> Result<CodingAgent, ExecutorError> {
    ExecutorConfigs::get_cached()
        .require_coding_agent(executor_profile_id)
        .map_err(|err| ExecutorError::UnknownExecutorType(err.to_string()))
}

#[async_trait]
pub trait Executable {
    async fn spawn(
        &self,
        current_dir: &Path,
        approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError>;
}

#[async_trait]
impl Executable for ExecutorAction {
    async fn spawn(
        &self,
        current_dir: &Path,
        approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        match &self.typ {
            ExecutorActionType::CodingAgentInitialRequest(request) => {
                request.spawn(current_dir, approvals, env).await
            }
            ExecutorActionType::CodingAgentFollowUpRequest(request) => {
                request.spawn(current_dir, approvals, env).await
            }
            ExecutorActionType::ScriptRequest(request) => {
                request.spawn(current_dir, approvals, env).await
            }
        }
    }
}

#[async_trait]
impl Executable for CodingAgentInitialRequest {
    async fn spawn(
        &self,
        current_dir: &Path,
        approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let effective_dir = match &self.working_dir {
            Some(rel_path) => current_dir.join(rel_path),
            None => current_dir.to_path_buf(),
        };

        let mut agent = resolve_coding_agent(&self.executor_profile_id)?;
        agent.use_approvals(approvals);

        match &agent {
            #[cfg(feature = "codex")]
            CodingAgent::Codex(codex) => {
                codex
                    .spawn_with_image_paths(
                        &effective_dir,
                        &self.prompt,
                        self.image_paths.as_ref(),
                        env,
                    )
                    .await
            }
            _ => agent.spawn(&effective_dir, &self.prompt, env).await,
        }
    }
}

#[async_trait]
impl Executable for CodingAgentFollowUpRequest {
    async fn spawn(
        &self,
        current_dir: &Path,
        approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let effective_dir = match &self.working_dir {
            Some(rel_path) => current_dir.join(rel_path),
            None => current_dir.to_path_buf(),
        };

        let mut agent = resolve_coding_agent(&self.executor_profile_id)?;
        agent.use_approvals(approvals);

        match &agent {
            #[cfg(feature = "codex")]
            CodingAgent::Codex(codex) => {
                codex
                    .spawn_follow_up_with_image_paths(
                        &effective_dir,
                        &self.prompt,
                        &self.session_id,
                        self.image_paths.as_ref(),
                        env,
                    )
                    .await
            }
            _ => {
                agent
                    .spawn_follow_up(&effective_dir, &self.prompt, &self.session_id, env)
                    .await
            }
        }
    }
}

#[async_trait]
impl Executable for ScriptRequest {
    async fn spawn(
        &self,
        current_dir: &Path,
        _approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // Use working_dir if specified, otherwise use current_dir
        let effective_dir = match &self.working_dir {
            Some(rel_path) => current_dir.join(rel_path),
            None => current_dir.to_path_buf(),
        };

        let mut command = match self.context {
            ScriptContext::DevServer => {
                let (program, args) = parse_direct_command(&self.script)?;
                let mut command = Command::new(program);
                command.args(args);
                command
            }
            _ => {
                let (shell_cmd, shell_arg) = get_shell_command();
                let mut command = Command::new(shell_cmd);
                command.arg(shell_arg).arg(&self.script);
                command
            }
        };

        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&effective_dir);

        env.apply_to_command(&mut command);

        let child = command.group_spawn()?;
        Ok(child.into())
    }
}

fn parse_direct_command(script: &str) -> Result<(String, Vec<String>), ExecutorError> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Err(ExecutorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Dev script command is empty",
        )));
    }

    let parts = shlex::split(trimmed).ok_or_else(|| {
        ExecutorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Dev script is not valid command text",
        ))
    })?;
    if parts.is_empty() {
        return Err(ExecutorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Dev script command is empty",
        )));
    }

    let has_forbidden_shell_operators = parts.iter().any(|part| {
        matches!(
            part.as_str(),
            "|" | "||" | "&" | "&&" | ";" | ">" | ">>" | "<" | "<<"
        )
    });
    if has_forbidden_shell_operators {
        return Err(ExecutorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Dev script must be a single command without shell operators",
        )));
    }

    let mut iter = parts.into_iter();
    let program = iter.next().ok_or_else(|| {
        ExecutorError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Dev script command is empty",
        ))
    })?;
    let args = iter.collect();
    Ok((program, args))
}

#[cfg(test)]
mod tests {
    use super::parse_direct_command;

    #[test]
    fn parse_direct_command_accepts_simple_command() {
        let parsed = parse_direct_command("npm run dev -- --port 3000").unwrap();
        assert_eq!(parsed.0, "npm");
        assert_eq!(parsed.1, vec!["run", "dev", "--", "--port", "3000"]);
    }

    #[test]
    fn parse_direct_command_rejects_shell_operators() {
        let err = parse_direct_command("npm run dev && rm -rf /").unwrap_err();
        assert!(err.to_string().contains("without shell operators"));
    }
}
