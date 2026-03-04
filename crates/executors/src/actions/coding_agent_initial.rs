use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use executors_protocol::actions::coding_agent_initial::CodingAgentInitialRequest;

use crate::{
    actions::Executable,
    approvals::ExecutorApprovalService,
    env::ExecutionEnv,
    executors::{ExecutorError, SpawnedChild, StandardCodingAgentExecutor},
    profile::ExecutorConfigs,
};

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

        let executor_profile_id = self.executor_profile_id.clone();
        let mut agent = ExecutorConfigs::get_cached()
            .get_coding_agent(&executor_profile_id)
            .ok_or(ExecutorError::UnknownExecutorType(
                executor_profile_id.to_string(),
            ))?;

        agent.use_approvals(approvals.clone());

        if let crate::executors::CodingAgent::Codex(codex) = &agent {
            return codex
                .spawn_with_image_paths(
                    &effective_dir,
                    &self.prompt,
                    self.image_paths.as_ref(),
                    env,
                )
                .await;
        }

        agent.spawn(&effective_dir, &self.prompt, env).await
    }
}
