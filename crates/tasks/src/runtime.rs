use async_trait::async_trait;
use db::models::workspace::Workspace;
use executors_protocol::ExecutorProfileId;
use uuid::Uuid;

#[async_trait]
pub trait TaskRuntime {
    async fn git_branch_from_workspace(&self, attempt_id: Uuid, task_title: &str) -> String;

    async fn start_workspace(
        &self,
        workspace: &Workspace,
        executor_profile_id: ExecutorProfileId,
        prompt_override: Option<String>,
    ) -> Result<(), String>;

    async fn delete_workspace_container(&self, workspace: &Workspace) -> Result<(), String>;

    async fn has_running_processes(&self, task_id: Uuid) -> Result<bool, String>;
}
