use app_runtime::Deployment;
#[cfg(test)]
use db::models::milestone::MilestoneGraph;
use db::models::{milestone::Milestone, task::Task};
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

pub async fn delete_milestone_with_cleanup(
    deployment: &DeploymentImpl,
    milestone: Milestone,
    entry_task_override: Option<Task>,
    allow_archived: bool,
) -> Result<(), ApiError> {
    let runtime = DeploymentTaskRuntime::new(deployment.container());
    domain::delete_milestone_with_cleanup(
        &runtime,
        &deployment.db().pool,
        milestone,
        entry_task_override,
        allow_archived,
    )
    .await
    .map_err(ApiError::from)
}

#[cfg(test)]
fn topo_sorted_task_ids(graph: &MilestoneGraph) -> Vec<uuid::Uuid> {
    domain::topo_sorted_task_ids(graph)
}

#[cfg(test)]
mod tests {
    use db::models::milestone::{
        MilestoneEdge, MilestoneGraph, MilestoneNode, MilestoneNodeBaseStrategy, MilestoneNodeKind,
        MilestoneNodeLayout,
    };
    use uuid::Uuid;

    use super::topo_sorted_task_ids;

    fn node(id: &str) -> (MilestoneNode, Uuid) {
        let task_id = Uuid::new_v4();
        (
            MilestoneNode {
                id: id.to_string(),
                task_id,
                kind: MilestoneNodeKind::Task,
                phase: 0,
                executor_profile_id: None,
                base_strategy: MilestoneNodeBaseStrategy::Topology,
                instructions: None,
                requires_approval: None,
                layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                status: None,
            },
            task_id,
        )
    }

    #[test]
    fn topo_sorted_task_ids_respects_edges() {
        let (node_a, task_id_a) = node("a");
        let (node_b, task_id_b) = node("b");
        let graph = MilestoneGraph {
            nodes: vec![node_a, node_b],
            edges: vec![MilestoneEdge {
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
