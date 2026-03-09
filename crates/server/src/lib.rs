pub mod api;
pub mod auto_orchestrator;
pub mod auto_orchestrator_prompt;
pub mod error;
pub mod http;
pub mod milestone_dispatch;
pub mod mcp;
pub mod middleware;
pub mod routes;
pub mod task_runtime;
#[cfg(test)]
pub mod test_support;

// #[cfg(feature = "cloud")]
// type DeploymentImpl = vibe_kanban_cloud::deployment::CloudDeployment;
// #[cfg(not(feature = "cloud"))]
pub type DeploymentImpl = app_runtime::AppRuntime;
