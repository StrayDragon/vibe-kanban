use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::{
    DbErr, TransactionTrait,
    entities::task,
    events::{EVENT_TASK_UPDATED, TaskEventPayload},
    models::{
        archived_kanban::{ArchivedKanban, ArchivedKanbanWithTaskCount},
        event_outbox::EventOutbox,
        project::Project,
        task::{Task, TaskKind, TaskStatus},
        task_group::{TaskGroup, TaskGroupError},
    },
};
use deployment::Deployment;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, sea_query::Expr};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils_core::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, routes::task_deletion};

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

fn ensure_statuses_non_empty(statuses: &[TaskStatus]) -> Result<(), ApiError> {
    if statuses.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one status is required".to_string(),
        ));
    }
    Ok(())
}

fn map_task_group_error(err: TaskGroupError) -> ApiError {
    match err {
        TaskGroupError::Database(db_err) => ApiError::Database(db_err),
        _ => ApiError::BadRequest(err.to_string()),
    }
}

pub async fn list_project_archived_kanbans(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ArchivedKanbanWithTaskCount>>>, ApiError> {
    let archives =
        ArchivedKanban::list_by_project_with_task_counts(&deployment.db().pool, project.id).await?;
    Ok(ResponseJson(ApiResponse::success(archives)))
}

pub async fn archive_project_kanban(
    Extension(project): Extension<Project>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<ArchiveProjectKanbanRequest>,
) -> Result<ResponseJson<ApiResponse<ArchiveProjectKanbanResponse>>, ApiError> {
    ensure_statuses_non_empty(&payload.statuses)?;

    let title =
        normalize_title(payload.title).unwrap_or_else(|| default_archive_title(chrono::Utc::now()));

    let pool = &deployment.db().pool;
    let project_row_id = db::models::ids::project_id_by_uuid(pool, project.id)
        .await?
        .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

    let mut selected = task::Entity::find()
        .filter(task::Column::ProjectId.eq(project_row_id))
        .filter(task::Column::ArchivedKanbanId.is_null())
        .filter(task::Column::Status.is_in(payload.statuses.clone()))
        .all(pool)
        .await?;

    if selected.is_empty() {
        return Err(ApiError::BadRequest(
            "No matching tasks to archive".to_string(),
        ));
    }

    // Expand any selected task group ids into full-group atomic selection.
    let mut group_row_ids: Vec<i64> = selected.iter().filter_map(|t| t.task_group_id).collect();
    group_row_ids.sort_unstable();
    group_row_ids.dedup();

    let mut expanded_group_tasks: Vec<task::Model> = Vec::new();
    if !group_row_ids.is_empty() {
        let group_tasks = task::Entity::find()
            .filter(task::Column::ProjectId.eq(project_row_id))
            .filter(task::Column::TaskGroupId.is_in(group_row_ids.clone()))
            .all(pool)
            .await?;

        // Reject archiving if any selected group is already split (some tasks already archived).
        for group_row_id in &group_row_ids {
            let split = group_tasks
                .iter()
                .any(|t| t.task_group_id == Some(*group_row_id) && t.archived_kanban_id.is_some());
            if split {
                return Err(ApiError::Conflict(
                    "Task group is already archived/split. Restore the group first before archiving again.".to_string(),
                ));
            }
        }

        expanded_group_tasks = group_tasks;
    }

    // Build final set: non-group selected + all tasks in selected groups
    let mut by_row_id: std::collections::HashMap<i64, task::Model> =
        std::collections::HashMap::new();
    for model in selected.drain(..) {
        if model.task_group_id.is_none() {
            by_row_id.insert(model.id, model);
        } else {
            // group tasks will come from expanded_group_tasks
            by_row_id.insert(model.id, model);
        }
    }
    for model in expanded_group_tasks.drain(..) {
        by_row_id.insert(model.id, model);
    }

    let mut to_archive: Vec<task::Model> = by_row_id.into_values().collect();
    to_archive.sort_by_key(|t| t.id);

    // Final safety: ensure all are active (unarchived)
    if to_archive.iter().any(|t| t.archived_kanban_id.is_some()) {
        return Err(ApiError::Conflict(
            "Some selected tasks are already archived. Restore them first.".to_string(),
        ));
    }

    // Reject if any task has running execution processes
    for task_model in &to_archive {
        if deployment
            .container()
            .has_running_processes(task_model.uuid)
            .await?
        {
            return Err(ApiError::Conflict(
                "Some selected tasks have running execution processes. Stop them first before archiving.".to_string(),
            ));
        }
    }

    let moved_task_count = u64::try_from(to_archive.len()).unwrap_or(u64::MAX);

    let tx = pool.begin().await?;
    let archive = ArchivedKanban::create(&tx, project.id, title).await?;
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
        return Err(ApiError::Conflict(
            "Some selected tasks changed state while archiving. Please refresh and try again."
                .to_string(),
        ));
    }

    for task_id in &task_uuids {
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: *task_id,
            project_id: project.id,
        })
        .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(&tx, EVENT_TASK_UPDATED, "task", *task_id, payload).await?;
    }

    tx.commit().await?;

    let archived_kanban = ArchivedKanbanWithTaskCount {
        archived_kanban: archive,
        tasks_count: moved_task_count,
    };

    Ok(ResponseJson(ApiResponse::success(
        ArchiveProjectKanbanResponse {
            archived_kanban,
            moved_task_count,
        },
    )))
}

pub async fn get_archived_kanban(
    State(deployment): State<DeploymentImpl>,
    Path(archive_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<GetArchivedKanbanResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let archive = ArchivedKanban::find_by_id(pool, archive_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Archived kanban not found".to_string()))?;

    let tasks_count = ArchivedKanban::tasks_count(pool, archive_id).await?;

    Ok(ResponseJson(ApiResponse::success(
        GetArchivedKanbanResponse {
            archived_kanban: ArchivedKanbanWithTaskCount {
                archived_kanban: archive,
                tasks_count,
            },
        },
    )))
}

pub async fn restore_archived_kanban(
    State(deployment): State<DeploymentImpl>,
    Path(archive_id): Path<Uuid>,
    Json(payload): Json<RestoreArchivedKanbanRequest>,
) -> Result<ResponseJson<ApiResponse<RestoreArchivedKanbanResponse>>, ApiError> {
    let restore_all = payload.restore_all.unwrap_or(false);
    let statuses = payload.statuses.unwrap_or_default();

    if restore_all && !statuses.is_empty() {
        return Err(ApiError::BadRequest(
            "Do not provide statuses when restore_all=true".to_string(),
        ));
    }
    if !restore_all {
        ensure_statuses_non_empty(&statuses)?;
    }

    let pool = &deployment.db().pool;
    let archive = ArchivedKanban::find_by_id(pool, archive_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Archived kanban not found".to_string()))?;
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
        return Ok(ResponseJson(ApiResponse::success(
            RestoreArchivedKanbanResponse {
                restored_task_count: 0,
            },
        )));
    }

    let mut group_row_ids: Vec<i64> = selected.iter().filter_map(|t| t.task_group_id).collect();
    group_row_ids.sort_unstable();
    group_row_ids.dedup();

    let mut by_row_id: std::collections::HashMap<i64, task::Model> = selected
        .into_iter()
        .filter(|t| t.task_group_id.is_none())
        .map(|t| (t.id, t))
        .collect();

    if !group_row_ids.is_empty() {
        let group_tasks = task::Entity::find()
            .filter(task::Column::TaskGroupId.is_in(group_row_ids.clone()))
            .all(pool)
            .await?;

        for group_row_id in &group_row_ids {
            let split = group_tasks.iter().any(|t| {
                t.task_group_id == Some(*group_row_id)
                    && t.archived_kanban_id != Some(archive_row_id)
            });
            if split {
                return Err(ApiError::Conflict(
                    "Task group is split across archives/active. Restore the group consistently before continuing.".to_string(),
                ));
            }
        }

        for model in group_tasks {
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
        return Err(ApiError::Conflict(
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

    Ok(ResponseJson(ApiResponse::success(
        RestoreArchivedKanbanResponse {
            restored_task_count,
        },
    )))
}

pub async fn delete_archived_kanban(
    State(deployment): State<DeploymentImpl>,
    Path(archive_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteArchivedKanbanResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let _archive = ArchivedKanban::find_by_id(pool, archive_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Archived kanban not found".to_string()))?;

    let tasks_with_status =
        Task::find_filtered_with_attempt_status(pool, None, true, Some(archive_id)).await?;
    let tasks: Vec<Task> = tasks_with_status.into_iter().map(|t| t.task).collect();

    // Reject delete if any contained task has running processes.
    for task in &tasks {
        if deployment
            .container()
            .has_running_processes(task.id)
            .await?
        {
            return Err(ApiError::Conflict(
                "Archived kanban contains tasks with running execution processes. Stop them first."
                    .to_string(),
            ));
        }
    }

    // Safety valve: reject if any task group is split outside this archive.
    let mut group_ids: Vec<Uuid> = tasks.iter().filter_map(|t| t.task_group_id).collect();
    group_ids.sort_unstable();
    group_ids.dedup();

    for group_id in &group_ids {
        let group_tasks = Task::find_by_task_group_id(pool, *group_id).await?;
        let split = group_tasks
            .iter()
            .any(|t| t.archived_kanban_id != Some(archive_id));
        if split {
            return Err(ApiError::Conflict(
                "Archived kanban contains a task group that is split outside this archive. Refusing to delete to prevent data loss.".to_string(),
            ));
        }
    }

    let deleted_task_count = u64::try_from(tasks.len()).unwrap_or(u64::MAX);

    // Delete groups first (once per group), then standalone tasks.
    for group_id in &group_ids {
        let task_group = TaskGroup::find_by_id(pool, *group_id)
            .await
            .map_err(map_task_group_error)?
            .ok_or_else(|| ApiError::BadRequest("Task group not found".to_string()))?;

        let entry_task_override = tasks
            .iter()
            .find(|t| t.task_kind == TaskKind::Group && t.task_group_id == Some(*group_id))
            .cloned();

        task_deletion::delete_task_group_with_cleanup(
            &deployment,
            task_group,
            entry_task_override,
            true,
        )
        .await?;
    }

    for task in tasks.into_iter().filter(|t| t.task_group_id.is_none()) {
        task_deletion::delete_task_with_cleanup_allow_archived(
            &deployment,
            task,
            task_deletion::DeleteTaskMode::CascadeGroup,
        )
        .await?;
    }

    let rows = ArchivedKanban::delete(pool, archive_id).await?;
    if rows == 0 {
        return Err(ApiError::NotFound("Archived kanban not found".to_string()));
    }

    Ok(ResponseJson(ApiResponse::success(
        DeleteArchivedKanbanResponse { deleted_task_count },
    )))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let archive_id_router = Router::new()
        .route("/", get(get_archived_kanban).delete(delete_archived_kanban))
        .route("/restore", post(restore_archived_kanban));

    let _ = deployment;
    Router::new().nest(
        "/archived-kanbans",
        Router::new().nest("/{archive_id}", archive_id_router),
    )
}
