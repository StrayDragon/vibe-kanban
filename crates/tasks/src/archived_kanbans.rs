use db::{
    DbErr, TransactionTrait,
    entities::task,
    events::{EVENT_TASK_UPDATED, TaskEventPayload},
    models::{
        archived_kanban::{ArchivedKanban, ArchivedKanbanWithTaskCount},
        event_outbox::EventOutbox,
        milestone::{Milestone, MilestoneError},
        task::{Task, TaskKind, TaskStatus},
    },
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{orchestration::TasksError, runtime::TaskRuntime, task_deletion};

#[derive(Debug, Deserialize, TS)]
pub struct ArchiveProjectKanbanRequest {
    pub statuses: Vec<TaskStatus>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, TS)]
pub struct ArchiveProjectKanbanResponse {
    pub archived_kanban: ArchivedKanbanWithTaskCount,
    pub moved_task_count: u64,
}

#[derive(Debug, Serialize, TS)]
pub struct GetArchivedKanbanResponse {
    pub archived_kanban: ArchivedKanbanWithTaskCount,
}

#[derive(Debug, Deserialize, TS)]
pub struct RestoreArchivedKanbanRequest {
    pub restore_all: Option<bool>,
    pub statuses: Option<Vec<TaskStatus>>,
}

#[derive(Debug, Serialize, TS)]
pub struct RestoreArchivedKanbanResponse {
    pub restored_task_count: u64,
}

#[derive(Debug, Serialize, TS)]
pub struct DeleteArchivedKanbanResponse {
    pub deleted_task_count: u64,
}

fn default_archive_title(now: chrono::DateTime<chrono::Utc>) -> String {
    format!("归档 {}", now.format("%Y-%m-%d %H:%M"))
}

fn normalize_title(input: Option<String>) -> Option<String> {
    input.and_then(|raw| {
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn ensure_statuses_non_empty(statuses: &[TaskStatus]) -> Result<(), TasksError> {
    if statuses.is_empty() {
        return Err(TasksError::BadRequest(
            "At least one status is required".to_string(),
        ));
    }
    Ok(())
}

fn map_milestone_error(err: MilestoneError) -> TasksError {
    match err {
        MilestoneError::Database(db_err) => TasksError::Database(db_err),
        _ => TasksError::BadRequest(err.to_string()),
    }
}

pub async fn list_project_archived_kanbans(
    db: &db::DbPool,
    project_id: Uuid,
) -> Result<Vec<ArchivedKanbanWithTaskCount>, TasksError> {
    ArchivedKanban::list_by_project_with_task_counts(db, project_id)
        .await
        .map_err(TasksError::from)
}

pub async fn archive_project_kanban<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    project_id: Uuid,
    payload: &ArchiveProjectKanbanRequest,
) -> Result<ArchiveProjectKanbanResponse, TasksError> {
    ensure_statuses_non_empty(&payload.statuses)?;

    let title = normalize_title(payload.title.clone())
        .unwrap_or_else(|| default_archive_title(chrono::Utc::now()));

    let pool = db;
    let project_row_id = db::models::ids::project_id_by_uuid(pool, project_id)
        .await?
        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

    let mut selected = task::Entity::find()
        .filter(task::Column::ProjectId.eq(project_row_id))
        .filter(task::Column::ArchivedKanbanId.is_null())
        .filter(task::Column::Status.is_in(payload.statuses.clone()))
        .all(pool)
        .await?;

    if selected.is_empty() {
        return Err(TasksError::BadRequest(
            "No matching tasks to archive".to_string(),
        ));
    }

    // Expand any selected milestone ids into full-milestone atomic selection.
    let mut milestone_row_ids: Vec<i64> = selected.iter().filter_map(|t| t.milestone_id).collect();
    milestone_row_ids.sort_unstable();
    milestone_row_ids.dedup();

    let mut expanded_milestone_tasks: Vec<task::Model> = Vec::new();
    if !milestone_row_ids.is_empty() {
        let milestone_tasks = task::Entity::find()
            .filter(task::Column::ProjectId.eq(project_row_id))
            .filter(task::Column::MilestoneId.is_in(milestone_row_ids.clone()))
            .all(pool)
            .await?;

        // Reject archiving if any selected milestone is already split (some tasks already archived).
        for milestone_row_id in &milestone_row_ids {
            let split = milestone_tasks.iter().any(|t| {
                t.milestone_id == Some(*milestone_row_id) && t.archived_kanban_id.is_some()
            });
            if split {
                return Err(TasksError::Conflict(
                    "Milestone is already archived/split. Restore the milestone first before archiving again.".to_string(),
                ));
            }
        }

        expanded_milestone_tasks = milestone_tasks;
    }

    // Build final set: non-milestone selected + all tasks in selected milestones
    let mut by_row_id: std::collections::HashMap<i64, task::Model> =
        std::collections::HashMap::new();
    for model in selected.drain(..) {
        if model.milestone_id.is_none() {
            by_row_id.insert(model.id, model);
        } else {
            // milestone tasks will come from expanded_milestone_tasks
            by_row_id.insert(model.id, model);
        }
    }
    for model in expanded_milestone_tasks.drain(..) {
        by_row_id.insert(model.id, model);
    }

    let mut to_archive: Vec<task::Model> = by_row_id.into_values().collect();
    to_archive.sort_by_key(|t| t.id);

    // Final safety: ensure all are active (unarchived)
    if to_archive.iter().any(|t| t.archived_kanban_id.is_some()) {
        return Err(TasksError::Conflict(
            "Some selected tasks are already archived. Restore them first.".to_string(),
        ));
    }

    // Reject if any task has running execution processes
    for task_model in &to_archive {
        if runtime
            .has_running_processes(task_model.uuid)
            .await
            .map_err(TasksError::Runtime)?
        {
            return Err(TasksError::Conflict(
                "Some selected tasks have running execution processes. Stop them first before archiving.".to_string(),
            ));
        }
    }

    let moved_task_count = u64::try_from(to_archive.len()).unwrap_or(u64::MAX);

    let tx = pool.begin().await?;
    let archive = ArchivedKanban::create(&tx, project_id, title).await?;
    let archive_row_id = ArchivedKanban::row_id_by_uuid(&tx, archive.id)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "Archived kanban not found".to_string(),
        ))?;

    let now = chrono::Utc::now();
    let task_row_ids: Vec<i64> = to_archive.iter().map(|t| t.id).collect();
    let task_uuids: Vec<Uuid> = to_archive.iter().map(|t| t.uuid).collect();

    let updated = task::Entity::update_many()
        .col_expr(
            task::Column::ArchivedKanbanId,
            Expr::value(Some(archive_row_id)),
        )
        .col_expr(task::Column::UpdatedAt, Expr::value(now))
        .filter(task::Column::ProjectId.eq(project_row_id))
        .filter(task::Column::Id.is_in(task_row_ids))
        .filter(task::Column::ArchivedKanbanId.is_null())
        .exec(&tx)
        .await?;

    if updated.rows_affected != moved_task_count {
        return Err(TasksError::Conflict(
            "Some selected tasks changed state while archiving. Please refresh and try again."
                .to_string(),
        ));
    }

    for task_id in &task_uuids {
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: *task_id,
            project_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(&tx, EVENT_TASK_UPDATED, "task", *task_id, payload).await?;
    }

    tx.commit().await?;

    let archived_kanban = ArchivedKanbanWithTaskCount {
        archived_kanban: archive,
        tasks_count: moved_task_count,
    };

    Ok(ArchiveProjectKanbanResponse {
        archived_kanban,
        moved_task_count,
    })
}

pub async fn get_archived_kanban(
    db: &db::DbPool,
    archive_id: Uuid,
) -> Result<GetArchivedKanbanResponse, TasksError> {
    let pool = db;
    let archive = ArchivedKanban::find_by_id(pool, archive_id)
        .await?
        .ok_or_else(|| TasksError::NotFound("Archived kanban not found".to_string()))?;

    let tasks_count = ArchivedKanban::tasks_count(pool, archive_id).await?;

    Ok(GetArchivedKanbanResponse {
        archived_kanban: ArchivedKanbanWithTaskCount {
            archived_kanban: archive,
            tasks_count,
        },
    })
}

pub async fn restore_archived_kanban<R: TaskRuntime + Sync>(
    _runtime: &R,
    db: &db::DbPool,
    archive_id: Uuid,
    payload: &RestoreArchivedKanbanRequest,
) -> Result<RestoreArchivedKanbanResponse, TasksError> {
    let restore_all = payload.restore_all.unwrap_or(false);
    let statuses = payload.statuses.clone().unwrap_or_default();

    if restore_all && !statuses.is_empty() {
        return Err(TasksError::BadRequest(
            "Do not provide statuses when restore_all=true".to_string(),
        ));
    }
    if !restore_all {
        ensure_statuses_non_empty(&statuses)?;
    }

    let pool = db;
    let archive = ArchivedKanban::find_by_id(pool, archive_id)
        .await?
        .ok_or_else(|| TasksError::NotFound("Archived kanban not found".to_string()))?;
    let archive_row_id = ArchivedKanban::row_id_by_uuid(pool, archive_id)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "Archived kanban not found".to_string(),
        ))?;

    let mut selected_query =
        task::Entity::find().filter(task::Column::ArchivedKanbanId.eq(archive_row_id));
    if !restore_all {
        selected_query = selected_query.filter(task::Column::Status.is_in(statuses.clone()));
    }
    let selected = selected_query.all(pool).await?;

    if selected.is_empty() {
        return Ok(RestoreArchivedKanbanResponse {
            restored_task_count: 0,
        });
    }

    let mut milestone_row_ids: Vec<i64> = selected.iter().filter_map(|t| t.milestone_id).collect();
    milestone_row_ids.sort_unstable();
    milestone_row_ids.dedup();

    let mut by_row_id: std::collections::HashMap<i64, task::Model> = selected
        .into_iter()
        .filter(|t| t.milestone_id.is_none())
        .map(|t| (t.id, t))
        .collect();

    if !milestone_row_ids.is_empty() {
        let milestone_tasks = task::Entity::find()
            .filter(task::Column::MilestoneId.is_in(milestone_row_ids.clone()))
            .all(pool)
            .await?;

        for milestone_row_id in &milestone_row_ids {
            let split = milestone_tasks.iter().any(|t| {
                t.milestone_id == Some(*milestone_row_id)
                    && t.archived_kanban_id != Some(archive_row_id)
            });
            if split {
                return Err(TasksError::Conflict(
                    "Milestone is split across archives/active. Restore the milestone consistently before continuing.".to_string(),
                ));
            }
        }

        for model in milestone_tasks {
            by_row_id.insert(model.id, model);
        }
    }

    let to_restore: Vec<task::Model> = by_row_id.into_values().collect();
    let restored_task_count = u64::try_from(to_restore.len()).unwrap_or(u64::MAX);

    let task_row_ids: Vec<i64> = to_restore.iter().map(|t| t.id).collect();
    let task_uuids: Vec<Uuid> = to_restore.iter().map(|t| t.uuid).collect();
    let now = chrono::Utc::now();

    let tx = pool.begin().await?;
    let updated = task::Entity::update_many()
        .col_expr(task::Column::ArchivedKanbanId, Expr::value(None::<i64>))
        .col_expr(task::Column::UpdatedAt, Expr::value(now))
        .filter(task::Column::Id.is_in(task_row_ids))
        .filter(task::Column::ArchivedKanbanId.eq(archive_row_id))
        .exec(&tx)
        .await?;

    if updated.rows_affected != restored_task_count {
        return Err(TasksError::Conflict(
            "Some selected tasks changed state while restoring. Please refresh and try again."
                .to_string(),
        ));
    }

    for task_id in &task_uuids {
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: *task_id,
            project_id: archive.project_id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(&tx, EVENT_TASK_UPDATED, "task", *task_id, payload).await?;
    }

    tx.commit().await?;

    Ok(RestoreArchivedKanbanResponse {
        restored_task_count,
    })
}

pub async fn delete_archived_kanban<R: TaskRuntime + Sync>(
    runtime: &R,
    db: &db::DbPool,
    archive_id: Uuid,
) -> Result<DeleteArchivedKanbanResponse, TasksError> {
    let pool = db;
    let _archive = ArchivedKanban::find_by_id(pool, archive_id)
        .await?
        .ok_or_else(|| TasksError::NotFound("Archived kanban not found".to_string()))?;

    let tasks_with_status =
        Task::find_filtered_with_attempt_status(pool, None, true, Some(archive_id)).await?;
    let tasks: Vec<Task> = tasks_with_status.into_iter().map(|t| t.task).collect();

    // Reject delete if any contained task has running processes.
    for task in &tasks {
        if runtime
            .has_running_processes(task.id)
            .await
            .map_err(TasksError::Runtime)?
        {
            return Err(TasksError::Conflict(
                "Archived kanban contains tasks with running execution processes. Stop them first."
                    .to_string(),
            ));
        }
    }

    // Safety valve: reject if any milestone is split outside this archive.
    let mut milestone_ids: Vec<Uuid> = tasks.iter().filter_map(|t| t.milestone_id).collect();
    milestone_ids.sort_unstable();
    milestone_ids.dedup();

    for milestone_id in &milestone_ids {
        let milestone_tasks = Task::find_by_milestone_id(pool, *milestone_id).await?;
        let split = milestone_tasks
            .iter()
            .any(|t| t.archived_kanban_id != Some(archive_id));
        if split {
            return Err(TasksError::Conflict(
                "Archived kanban contains a milestone that is split outside this archive. Refusing to delete to prevent data loss.".to_string(),
            ));
        }
    }

    let deleted_task_count = u64::try_from(tasks.len()).unwrap_or(u64::MAX);

    // Delete milestones first (once per milestone), then standalone tasks.
    for milestone_id in &milestone_ids {
        let milestone = Milestone::find_by_id(pool, *milestone_id)
            .await
            .map_err(map_milestone_error)?
            .ok_or_else(|| TasksError::BadRequest("Milestone not found".to_string()))?;

        let entry_task_override = tasks
            .iter()
            .find(|t| t.task_kind == TaskKind::Milestone && t.milestone_id == Some(*milestone_id))
            .cloned();

        task_deletion::delete_milestone_with_cleanup(
            runtime,
            db,
            milestone,
            entry_task_override,
            true,
        )
        .await?;
    }

    for task in tasks.into_iter().filter(|t| t.milestone_id.is_none()) {
        task_deletion::delete_task_with_cleanup(
            runtime,
            db,
            task,
            task_deletion::DeleteTaskMode::CascadeMilestone,
            true,
        )
        .await?;
    }

    let rows = ArchivedKanban::delete(pool, archive_id).await?;
    if rows == 0 {
        return Err(TasksError::NotFound(
            "Archived kanban not found".to_string(),
        ));
    }

    Ok(DeleteArchivedKanbanResponse { deleted_task_count })
}
