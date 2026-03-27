use app_runtime::Deployment;
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessRunReason},
    session::{CreateSession, Session},
    workspace::Workspace,
};
use execution::container::ContainerService;
use executors::{
    agent_command::{AgentCommandKey, agent_command_resolver, command_identity_for_agent},
    command::{CommandBuilder, apply_overrides},
    executors::{ExecutorError, codex::Codex},
};
use executors_protocol::{
    BaseCodingAgent,
    actions::{
        ExecutorAction, ExecutorActionType,
        script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
    },
};
use uuid::Uuid;

use crate::error::ApiError;

fn join_program_and_args_for_bash(
    program_path: &std::path::Path,
    args: &[String],
) -> Result<String, ApiError> {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program_path.to_string_lossy().to_string());
    parts.extend(args.iter().cloned());

    shlex::try_join(parts.iter().map(|s| s.as_str()))
        .map_err(|err| ApiError::Internal(format!("Failed to quote argv for bash: {err}")))
}

pub async fn run_codex_setup(
    deployment: &crate::DeploymentImpl,
    workspace: &Workspace,
    codex: &Codex,
) -> Result<ExecutionProcess, ApiError> {
    let latest_process = ExecutionProcess::find_latest_by_workspace_and_run_reason(
        &deployment.db().pool,
        workspace.id,
        &ExecutionProcessRunReason::CodingAgent,
    )
    .await?;

    let executor_action = if let Some(latest_process) = latest_process {
        let latest_action = latest_process.executor_action();
        get_setup_helper_action(codex)
            .await?
            .append_action(latest_action.to_owned())
    } else {
        get_setup_helper_action(codex).await?
    };

    deployment
        .container()
        .ensure_container_exists(workspace)
        .await?;

    // Get or create a session for setup scripts
    let session =
        match Session::find_latest_by_workspace_id(&deployment.db().pool, workspace.id).await? {
            Some(s) => s,
            None => {
                // Create a new session for setup scripts
                Session::create(
                    &deployment.db().pool,
                    &CreateSession {
                        executor: Some("codex".to_string()),
                    },
                    Uuid::new_v4(),
                    workspace.id,
                )
                .await?
            }
        };

    let execution_process = deployment
        .container()
        .start_execution(
            workspace,
            &session,
            &executor_action,
            &ExecutionProcessRunReason::SetupScript,
        )
        .await?;
    Ok(execution_process)
}

async fn get_setup_helper_action(codex: &Codex) -> Result<ExecutorAction, ApiError> {
    let resolved = agent_command_resolver()
        .resolve_with_overrides(
            AgentCommandKey::Agent(BaseCodingAgent::Codex),
            command_identity_for_agent(BaseCodingAgent::Codex),
            &codex.cmd,
        )
        .await;
    let mut login_command = CommandBuilder::new(resolved.base_command);
    login_command = login_command.extend_params(["login"]);
    login_command = apply_overrides(login_command, &codex.cmd);

    let (program_path, args) = login_command
        .build_initial()
        .map_err(|err| ApiError::Executor(ExecutorError::from(err)))?
        .into_resolved()
        .await
        .map_err(ApiError::Executor)?;
    let login_script = join_program_and_args_for_bash(&program_path, &args)?;
    let login_request = ScriptRequest {
        script: login_script,
        language: ScriptRequestLanguage::Bash,
        context: ScriptContext::ToolInstallScript,
        working_dir: None,
    };

    Ok(ExecutorAction::new(
        ExecutorActionType::ScriptRequest(login_request),
        None,
    ))
}

#[cfg(test)]
mod tests {
    use super::join_program_and_args_for_bash;

    #[test]
    fn quoted_script_preserves_argv_semantics() {
        let program = std::path::PathBuf::from("/tmp/weird path/bin");
        let args = vec![
            "--flag".to_string(),
            "with spaces".to_string(),
            "quote'quote".to_string(),
            "semi;colon".to_string(),
            "new\nline".to_string(),
            "\"double\"".to_string(),
            "$(echo injected)".to_string(),
        ];

        let script = join_program_and_args_for_bash(&program, &args).expect("quoted script");
        let parts = shlex::split(&script).expect("quoted script should be parseable");

        assert_eq!(parts.first().map(String::as_str), Some("/tmp/weird path/bin"));
        assert_eq!(&parts[1..], args.as_slice());
    }
}
