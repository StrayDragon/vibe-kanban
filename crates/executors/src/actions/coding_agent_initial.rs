use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    actions::Executable,
    approvals::ExecutorApprovalService,
    env::ExecutionEnv,
    executors::{BaseCodingAgent, ExecutorError, SpawnedChild, StandardCodingAgentExecutor},
    profile::{ExecutorConfigs, ExecutorProfileId},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct CodingAgentInitialRequest {
    pub prompt: String,
    /// Executor profile specification
    #[serde(alias = "profile_variant_label")]
    // Backwards compatability with ProfileVariantIds, esp stored in DB under ExecutorAction
    pub executor_profile_id: ExecutorProfileId,
    /// Optional relative path to execute the agent in (relative to container_ref).
    /// If None, uses the container_ref directory directly.
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Optional image path map keyed by prompt image src.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_paths: Option<HashMap<String, PathBuf>>,
}

impl CodingAgentInitialRequest {
    pub fn base_executor(&self) -> BaseCodingAgent {
        self.executor_profile_id.executor
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
        // Use working_dir if specified, otherwise use current_dir
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
