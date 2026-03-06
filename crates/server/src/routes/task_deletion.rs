#[cfg(test)]
use db::models::task_group::TaskGroupGraph;
use db::models::{task::Task, task_group::TaskGroup};
use app_runtime::Deployment;
pub use domain::DeleteTaskMode;
use tasks::task_deletion as domain;

use crate::{DeploymentImpl, error::ApiError, task_runtime::DeploymentTaskRuntime};

pub async fn delete_task_with_cleanup(
    deployment: &DeploymentImpl,
    task: Task,
    mode: DeleteTaskMode,
) -> Result<(), ApiError> {
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    domain::delete_task_with_cleanup(&runtime, &deployment.db().pool, task, mode, false)
        .await
        .map_err(ApiError::from)
}

pub async fn delete_task_group_with_cleanup(
    deployment: &DeploymentImpl,
    task_group: TaskGroup,
    entry_task_override: Option<Task>,
    allow_archived: bool,
) -> Result<(), ApiError> {
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    domain::delete_task_group_with_cleanup(
        &runtime,
        &deployment.db().pool,
        task_group,
        entry_task_override,
        allow_archived,
    )
    .await
    .map_err(ApiError::from)
}

#[cfg(test)]
fn topo_sorted_task_ids(graph: &TaskGroupGraph) -> Vec<uuid::Uuid> {
    domain::topo_sorted_task_ids(graph)
}

#[cfg(test)]
mod tests {
    use db::models::task_group::{
        TaskGroupEdge, TaskGroupGraph, TaskGroupNode, TaskGroupNodeBaseStrategy, TaskGroupNodeKind,
        TaskGroupNodeLayout,
    };
    use uuid::Uuid;

    use super::topo_sorted_task_ids;

    fn node(id: &str) -> (TaskGroupNode, Uuid) {
        let task_id = Uuid::new_v4();
        (
            TaskGroupNode {
                id: id.to_string(),
                task_id,
                kind: TaskGroupNodeKind::Task,
                phase: 0,
                executor_profile_id: None,
                base_strategy: TaskGroupNodeBaseStrategy::Topology,
                instructions: None,
                requires_approval: None,
                layout: TaskGroupNodeLayout { x: 0.0, y: 0.0 },
                status: None,
            },
            task_id,
        )
    }

    #[test]
    fn topo_sorted_task_ids_respects_edges() {
        let (node_a, task_id_a) = node("a");
        let (node_b, task_id_b) = node("b");
        let graph = TaskGroupGraph {
            nodes: vec![node_a, node_b],
            edges: vec![TaskGroupEdge {
                id: "edge-a-b".to_string(),
                from: "a".to_string(),
                to: "b".to_string(),
                data_flow: None,
            }],
        };

        let order = topo_sorted_task_ids(&graph);
        let idx_a = order.iter().position(|id| *id == task_id_a).unwrap();
        let idx_b = order.iter().position(|id| *id == task_id_b).unwrap();
        assert!(idx_a < idx_b);
    }
}
