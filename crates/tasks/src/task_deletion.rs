use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};

use db::{
    DbErr, TransactionTrait,
    models::{
        repo::Repo,
        task::{Task, TaskKind},
        task_group::{TaskGroup, TaskGroupError, TaskGroupGraph},
        workspace::Workspace,
        workspace_repo::WorkspaceRepo,
    },
};
use repos::workspace_manager::WorkspaceManager;

use crate::{orchestration::TasksError, runtime::TaskRuntime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteTaskMode {
    CascadeGroup,
}

fn map_task_group_error(err: TaskGroupError) -> TasksError {
    match err {
        TaskGroupError::Database(db_err) => TasksError::Database(db_err),
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
    if mode == DeleteTaskMode::CascadeGroup
        && task.task_kind == TaskKind::Group
        && let Some(task_group_id) = task.task_group_id
    {
        let task_group = TaskGroup::find_by_id(db, task_group_id)
            .await
            .map_err(map_task_group_error)?;
        if let Some(task_group) = task_group {
            return delete_task_group_with_cleanup(
                runtime,
                db,
                task_group,
                Some(task),
                allow_archived,
            )
            .await;
        }
    }

    delete_single_task_with_cleanup(runtime, db, task, allow_archived).await
}

pub async fn delete_task_group_with_cleanup<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    task_group: TaskGroup,
    entry_task_override: Option<Task>,
    allow_archived: bool,
) -> Result<(), TasksError> {
    let tasks = Task::find_by_task_group_id(db, task_group.id).await?;
    let mut entry_task = entry_task_override;
    let mut node_tasks = Vec::new();

    for task in tasks {
        if task.task_kind == TaskKind::Group {
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
                "Task group has running execution processes. Please stop them first.".to_string(),
            ));
        }
    }

    let ordered_task_ids = topo_sorted_task_ids(&task_group.graph);
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

    let rows = TaskGroup::delete(db, task_group.id)
        .await
        .map_err(map_task_group_error)?;
    if rows == 0 {
        return Err(TasksError::BadRequest("Task group not found".to_string()));
    }

    Ok(())
}

pub fn topo_sorted_task_ids(graph: &TaskGroupGraph) -> Vec<uuid::Uuid> {
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

    let repositories: Vec<Repo> = WorkspaceRepo::find_unique_repos_for_task(db, task.id).await?;
    let workspace_dirs: Vec<PathBuf> = attempts
        .iter()
        .filter_map(|attempt| attempt.container_ref.as_ref().map(PathBuf::from))
        .collect();

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
        tracing::info!(
            "Starting background cleanup for task {} ({} workspaces, {} repos)",
            task_id,
            workspace_dirs.len(),
            repositories.len()
        );

        for workspace_dir in &workspace_dirs {
            if let Err(err) =
                WorkspaceManager::cleanup_workspace(workspace_dir, &repositories).await
            {
                tracing::error!(
                    "Background workspace cleanup failed for task {} at {}: {}",
                    task_id,
                    workspace_dir.display(),
                    err
                );
            }
        }

        match Repo::delete_orphaned(&db).await {
            Ok(count) if count > 0 => {
                tracing::info!("Deleted {} orphaned repo records", count);
            }
            Err(err) => {
                tracing::error!("Failed to delete orphaned repos: {}", err);
            }
            _ => {}
        }

        tracing::info!("Background cleanup completed for task {}", task_id);
    });

    Ok(())
}
