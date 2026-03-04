use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use executors_protocol::actions::ExecutorAction;

use crate::{
    approvals::ExecutorApprovalService,
    env::ExecutionEnv,
    executors::{ExecutorError, SpawnedChild},
};
pub mod coding_agent_follow_up;
pub mod coding_agent_initial;
pub mod script;

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
            executors_protocol::actions::ExecutorActionType::CodingAgentInitialRequest(request) => {
                request.spawn(current_dir, approvals, env).await
            }
            executors_protocol::actions::ExecutorActionType::CodingAgentFollowUpRequest(
                request,
            ) => request.spawn(current_dir, approvals, env).await,
            executors_protocol::actions::ExecutorActionType::ScriptRequest(request) => {
                request.spawn(current_dir, approvals, env).await
            }
        }
    }
}
