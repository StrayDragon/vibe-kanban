pub mod api;
pub mod error;
pub mod http;
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
