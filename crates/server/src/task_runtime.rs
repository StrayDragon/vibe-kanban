use async_trait::async_trait;
use db::models::workspace::Workspace;
use execution::container::ContainerService;
use executors_protocol::ExecutorProfileId;
use tasks::runtime::TaskRuntime;
use uuid::Uuid;

pub struct DeploymentTaskRuntime<'a, C> {
    container: &'a C,
}

impl<'a, C> DeploymentTaskRuntime<'a, C> {
    pub fn new(container: &'a C) -> Self {
        Self { container }
    }
}

#[async_trait]
impl<C> TaskRuntime for DeploymentTaskRuntime<'_, C>
where
    C: ContainerService + Sync,
{
    async fn git_branch_from_workspace(&self, attempt_id: Uuid, task_title: &str) -> String {
        self.container
            .git_branch_from_workspace(&attempt_id, task_title)
            .await
    }

    async fn start_workspace(
        &self,
        workspace: &Workspace,
        executor_profile_id: ExecutorProfileId,
        prompt_override: Option<String>,
    ) -> Result<(), String> {
        self.container
            .start_workspace(workspace, executor_profile_id, prompt_override)
            .await
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    async fn delete_workspace_container(&self, workspace: &Workspace) -> Result<(), String> {
        self.container
            .delete(workspace)
            .await
            .map_err(|err| err.to_string())
    }

    async fn has_running_processes(&self, task_id: Uuid) -> Result<bool, String> {
        self.container
            .has_running_processes(task_id)
            .await
            .map_err(|err| err.to_string())
    }
}
