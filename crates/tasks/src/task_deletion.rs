use std::collections::{HashMap, VecDeque};

use db::{
    DbErr, TransactionTrait,
    models::{
        milestone::{Milestone, MilestoneError, MilestoneGraph},
        repo::Repo,
        task::{Task, TaskKind},
        workspace::Workspace,
    },
};

use crate::{orchestration::TasksError, runtime::TaskRuntime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteTaskMode {
    CascadeMilestone,
}

fn map_milestone_error(err: MilestoneError) -> TasksError {
    match err {
        MilestoneError::Database(db_err) => TasksError::Database(db_err),
        _ => TasksError::BadRequest(err.to_string()),
    }
}

pub async fn delete_task_with_cleanup<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    task: Task,
    mode: DeleteTaskMode,
    allow_archived: bool,
) -> Result<(), TasksError> {
    if mode == DeleteTaskMode::CascadeMilestone
        && task.task_kind == TaskKind::Milestone
        && let Some(milestone_id) = task.milestone_id
    {
        let milestone = Milestone::find_by_id(db, milestone_id)
            .await
            .map_err(map_milestone_error)?;
        if let Some(milestone) = milestone {
            return delete_milestone_with_cleanup(
                runtime,
                db,
                milestone,
                Some(task),
                allow_archived,
            )
            .await;
        }
    }

    delete_single_task_with_cleanup(runtime, db, task, allow_archived).await
}

pub async fn delete_milestone_with_cleanup<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    milestone: Milestone,
    entry_task_override: Option<Task>,
    allow_archived: bool,
) -> Result<(), TasksError> {
    let tasks = Task::find_by_milestone_id(db, milestone.id).await?;
    let mut entry_task = entry_task_override;
    let mut node_tasks = Vec::new();

    for task in tasks {
        if task.task_kind == TaskKind::Milestone {
            if entry_task.is_none() {
                entry_task = Some(task);
            }
        } else {
            node_tasks.push(task);
        }
    }

    let mut task_ids: Vec<_> = node_tasks.iter().map(|task| task.id).collect();
    if let Some(task) = entry_task.as_ref() {
        task_ids.push(task.id);
    }

    for task_id in task_ids {
        if runtime
            .has_running_processes(task_id)
            .await
            .map_err(TasksError::Runtime)?
        {
            return Err(TasksError::Conflict(
                "Milestone has running execution processes. Please stop them first.".to_string(),
            ));
        }
    }

    let ordered_task_ids = topo_sorted_task_ids(&milestone.graph);
    let mut tasks_by_id: HashMap<_, _> =
        node_tasks.into_iter().map(|task| (task.id, task)).collect();

    for task_id in ordered_task_ids.into_iter().rev() {
        if let Some(task) = tasks_by_id.remove(&task_id) {
            delete_single_task_with_cleanup(runtime, db, task, allow_archived).await?;
        }
    }

    for (_, task) in tasks_by_id {
        delete_single_task_with_cleanup(runtime, db, task, allow_archived).await?;
    }

    if let Some(task) = entry_task {
        delete_single_task_with_cleanup(runtime, db, task, allow_archived).await?;
    }

    let rows = Milestone::delete(db, milestone.id)
        .await
        .map_err(map_milestone_error)?;
    if rows == 0 {
        return Err(TasksError::BadRequest("Milestone not found".to_string()));
    }

    Ok(())
}

pub fn topo_sorted_task_ids(graph: &MilestoneGraph) -> Vec<uuid::Uuid> {
    let mut incoming: HashMap<String, usize> = HashMap::new();
    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
    let mut node_task_ids: HashMap<String, uuid::Uuid> = HashMap::new();

    for node in &graph.nodes {
        let node_id = node.id.trim().to_string();
        incoming.insert(node_id.clone(), 0);
        outgoing.insert(node_id.clone(), Vec::new());
        node_task_ids.insert(node_id, node.task_id);
    }

    for edge in &graph.edges {
        let from = edge.from.trim();
        let to = edge.to.trim();
        if !incoming.contains_key(from) || !incoming.contains_key(to) {
            continue;
        }
        if let Some(count) = incoming.get_mut(to) {
            *count += 1;
        }
        if let Some(list) = outgoing.get_mut(from) {
            list.push(to.to_string());
        }
    }

    let mut queue: VecDeque<String> = incoming
        .iter()
        .filter_map(|(node_id, count)| {
            if *count == 0 {
                Some(node_id.clone())
            } else {
                None
            }
        })
        .collect();
    let mut ordered_nodes = Vec::with_capacity(incoming.len());

    while let Some(node_id) = queue.pop_front() {
        ordered_nodes.push(node_id.clone());
        if let Some(children) = outgoing.get(&node_id) {
            for child in children {
                if let Some(count) = incoming.get_mut(child) {
                    *count -= 1;
                    if *count == 0 {
                        queue.push_back(child.clone());
                    }
                }
            }
        }
    }

    if ordered_nodes.len() != incoming.len() {
        return graph.nodes.iter().map(|node| node.task_id).collect();
    }

    ordered_nodes
        .into_iter()
        .filter_map(|node_id| node_task_ids.get(&node_id).copied())
        .collect()
}

async fn delete_single_task_with_cleanup<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    task: Task,
    allow_archived: bool,
) -> Result<(), TasksError> {
    if runtime
        .has_running_processes(task.id)
        .await
        .map_err(TasksError::Runtime)?
    {
        return Err(TasksError::Conflict(
            "Task has running execution processes. Please wait for them to complete or stop them first.".to_string(),
        ));
    }

    let attempts = Workspace::fetch_all(db, Some(task.id))
        .await
        .map_err(|err| {
            tracing::error!(
                "Failed to fetch task attempts for task {}: {}",
                task.id,
                err
            );
            TasksError::Workspace(err)
        })?;

    for attempt in &attempts {
        runtime
            .delete_workspace_container(attempt)
            .await
            .map_err(TasksError::Conflict)?;
    }

    let tx = db.begin().await?;
    let mut total_children_affected = 0u64;
    for attempt in &attempts {
        let children_affected = Task::nullify_children_by_workspace_id(&tx, attempt.id).await?;
        total_children_affected += children_affected;
    }

    let rows_affected = if allow_archived {
        Task::delete_allow_archived(&tx, task.id).await?
    } else {
        Task::delete(&tx, task.id).await?
    };

    if rows_affected == 0 {
        return Err(TasksError::Database(DbErr::RecordNotFound(
            "Task not found".to_string(),
        )));
    }

    tx.commit().await?;

    if total_children_affected > 0 {
        tracing::info!(
            "Nullified {} child task references before deleting task {}",
            total_children_affected,
            task.id
        );
    }

    let task_id = task.id;
    let db = db.clone();
    tokio::spawn(async move {
        match Repo::delete_orphaned(&db).await {
            Ok(count) if count > 0 => {
                tracing::info!(
                    "Deleted {} orphaned repo records after deleting task {}",
                    count,
                    task_id
                );
            }
            Err(err) => {
                tracing::error!(
                    "Failed to delete orphaned repos after deleting task {}: {}",
                    task_id,
                    err
                );
            }
            _ => {}
        }
    });

    Ok(())
}
