use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
    sea_query::{Expr, ExprTrait, JoinType, Order, Query},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{project, task, workspace},
    events::{
        EVENT_PROJECT_CREATED, EVENT_PROJECT_DELETED, EVENT_PROJECT_UPDATED, EVENT_TASK_DELETED,
        ProjectEventPayload, TaskEventPayload,
    },
    models::event_outbox::EventOutbox,
    types::{WorkspaceLifecycleHookFailurePolicy, WorkspaceLifecycleHookRunMode},
};

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error("Project not found")]
    ProjectNotFound,
    #[error("Failed to create project: {0}")]
    CreateFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub dev_script: Option<String>,
    pub dev_script_working_dir: Option<String>,
    pub default_agent_working_dir: Option<String>,
    pub git_no_verify_override: Option<bool>,
    pub scheduler_max_concurrent: i32,
    pub scheduler_max_retries: i32,
    pub default_continuation_turns: i32,
    pub after_prepare_hook: Option<WorkspaceLifecycleHookConfig>,
    pub before_cleanup_hook: Option<WorkspaceLifecycleHookConfig>,
    pub remote_project_id: Option<Uuid>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    #[ts(type = "Date")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct WorkspaceLifecycleHookConfig {
    pub command: String,
    pub working_dir: Option<String>,
    pub failure_policy: WorkspaceLifecycleHookFailurePolicy,
    pub run_mode: Option<WorkspaceLifecycleHookRunMode>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateProject {
    pub name: String,
    pub repositories: Vec<super::project_repo::CreateProjectRepo>,
}

#[derive(Debug, Deserialize, TS)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub dev_script: Option<String>,
    pub dev_script_working_dir: Option<String>,
    pub default_agent_working_dir: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_bool_as_double_option")]
    pub git_no_verify_override: Option<Option<bool>>,
    pub scheduler_max_concurrent: Option<i32>,
    pub scheduler_max_retries: Option<i32>,
    pub default_continuation_turns: Option<i32>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_hook_config_as_double_option"
    )]
    pub after_prepare_hook: Option<Option<WorkspaceLifecycleHookConfig>>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_hook_config_as_double_option"
    )]
    pub before_cleanup_hook: Option<Option<WorkspaceLifecycleHookConfig>>,
}

fn deserialize_optional_bool_as_double_option<'de, D>(
    deserializer: D,
) -> Result<Option<Option<bool>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<bool>::deserialize(deserializer)?))
}

fn deserialize_optional_hook_config_as_double_option<'de, D>(
    deserializer: D,
) -> Result<Option<Option<WorkspaceLifecycleHookConfig>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<WorkspaceLifecycleHookConfig>::deserialize(
        deserializer,
    )?))
}

#[derive(Debug, Serialize, TS)]
pub struct SearchResult {
    pub path: String,
    pub is_file: bool,
    pub match_type: SearchMatchType,
}

#[derive(Debug, Serialize, TS)]
pub struct ProjectFileSearchResponse {
    pub results: Vec<SearchResult>,
    pub index_truncated: bool,
    pub truncated_repos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub enum SearchMatchType {
    FileName,
    DirectoryName,
    FullPath,
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn build_hook_config(
    command: Option<String>,
    working_dir: Option<String>,
    failure_policy: Option<WorkspaceLifecycleHookFailurePolicy>,
    run_mode: Option<WorkspaceLifecycleHookRunMode>,
) -> Option<WorkspaceLifecycleHookConfig> {
    let command = normalize_optional_string(command)?;
    Some(WorkspaceLifecycleHookConfig {
        command,
        working_dir: normalize_optional_string(working_dir),
        failure_policy: failure_policy.unwrap_or(WorkspaceLifecycleHookFailurePolicy::WarnOnly),
        run_mode,
    })
}

fn apply_hook_update(
    active: &mut project::ActiveModel,
    hook: Option<WorkspaceLifecycleHookConfig>,
    is_after_prepare: bool,
) {
    let (command, working_dir, failure_policy, run_mode) = match hook {
        Some(hook) => (
            Some(hook.command),
            hook.working_dir,
            Some(hook.failure_policy),
            hook.run_mode,
        ),
        None => (None, None, None, None),
    };

    if is_after_prepare {
        active.after_prepare_hook_command = Set(command);
        active.after_prepare_hook_working_dir = Set(working_dir);
        active.after_prepare_hook_failure_policy = Set(failure_policy);
        active.after_prepare_hook_run_mode = Set(run_mode);
    } else {
        active.before_cleanup_hook_command = Set(command);
        active.before_cleanup_hook_working_dir = Set(working_dir);
        active.before_cleanup_hook_failure_policy = Set(failure_policy);
    }
}

impl Project {
    fn from_model(model: project::Model) -> Self {
        Self {
            id: model.uuid,
            name: model.name,
            dev_script: model.dev_script,
            dev_script_working_dir: model.dev_script_working_dir,
            default_agent_working_dir: model.default_agent_working_dir,
            git_no_verify_override: model.git_no_verify_override,
            scheduler_max_concurrent: model.scheduler_max_concurrent,
            scheduler_max_retries: model.scheduler_max_retries,
            default_continuation_turns: model.default_continuation_turns,
            after_prepare_hook: build_hook_config(
                model.after_prepare_hook_command,
                model.after_prepare_hook_working_dir,
                model.after_prepare_hook_failure_policy,
                model.after_prepare_hook_run_mode,
            ),
            before_cleanup_hook: build_hook_config(
                model.before_cleanup_hook_command,
                model.before_cleanup_hook_working_dir,
                model.before_cleanup_hook_failure_policy,
                None,
            ),
            remote_project_id: model.remote_project_id,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }

    pub fn effective_git_no_verify(&self, global_default: bool) -> bool {
        self.git_no_verify_override.unwrap_or(global_default)
    }

    pub async fn count<C: ConnectionTrait>(db: &C) -> Result<i64, DbErr> {
        let count = project::Entity::find().count(db).await?;
        Ok(i64::try_from(count).unwrap_or(i64::MAX))
    }

    pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let records = project::Entity::find()
            .order_by_desc(project::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(records.into_iter().map(Self::from_model).collect())
    }

    /// Find the most actively used projects based on recent task activity
    pub async fn find_most_active<C: ConnectionTrait>(
        db: &C,
        limit: i32,
    ) -> Result<Vec<Self>, DbErr> {
        let query = Query::select()
            .column(task::Column::ProjectId)
            .from(task::Entity)
            .join(
                JoinType::InnerJoin,
                workspace::Entity,
                Expr::col((workspace::Entity, workspace::Column::TaskId))
                    .equals((task::Entity, task::Column::Id)),
            )
            .order_by(
                (workspace::Entity, workspace::Column::UpdatedAt),
                Order::Desc,
            )
            .distinct()
            .limit(std::cmp::max(limit, 0) as u64)
            .to_owned();

        let rows = db.query_all(&query).await?;
        let mut project_ids = Vec::with_capacity(rows.len());
        for row in rows {
            if let Ok(id) = row.try_get("", "project_id") {
                project_ids.push(id);
            }
        }

        if project_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut projects = project::Entity::find()
            .filter(project::Column::Id.is_in(project_ids.clone()))
            .all(db)
            .await?
            .into_iter()
            .map(|model| (model.id, Self::from_model(model)))
            .collect::<HashMap<_, _>>();

        let mut ordered = Vec::with_capacity(project_ids.len());
        for id in project_ids {
            if let Some(project) = projects.remove(&id) {
                ordered.push(project);
            }
        }

        Ok(ordered)
    }

    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<Option<Self>, DbErr> {
        let record = project::Entity::find()
            .filter(project::Column::Uuid.eq(id))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn find_by_remote_project_id<C: ConnectionTrait>(
        db: &C,
        remote_project_id: Uuid,
    ) -> Result<Option<Self>, DbErr> {
        let record = project::Entity::find()
            .filter(project::Column::RemoteProjectId.eq(remote_project_id))
            .one(db)
            .await?;
        Ok(record.map(Self::from_model))
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateProject,
        project_id: Uuid,
    ) -> Result<Self, DbErr> {
        let now = Utc::now();
        let active = project::ActiveModel {
            uuid: Set(project_id),
            name: Set(data.name.clone()),
            dev_script: Set(None),
            dev_script_working_dir: Set(None),
            default_agent_working_dir: Set(None),
            git_no_verify_override: Set(None),
            scheduler_max_concurrent: Set(1),
            scheduler_max_retries: Set(3),
            default_continuation_turns: Set(0),
            after_prepare_hook_command: Set(None),
            after_prepare_hook_working_dir: Set(None),
            after_prepare_hook_failure_policy: Set(None),
            after_prepare_hook_run_mode: Set(None),
            before_cleanup_hook_command: Set(None),
            before_cleanup_hook_working_dir: Set(None),
            before_cleanup_hook_failure_policy: Set(None),
            remote_project_id: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;
        let payload = serde_json::to_value(ProjectEventPayload { project_id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_PROJECT_CREATED, "project", project_id, payload).await?;
        Ok(Self::from_model(model))
    }

    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        payload: &UpdateProject,
    ) -> Result<Self, DbErr> {
        let record = project::Entity::find()
            .filter(project::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;

        let mut active: project::ActiveModel = record.into();
        if let Some(name) = payload.name.clone() {
            active.name = Set(name);
        }
        if payload.dev_script.is_some() {
            active.dev_script = Set(payload.dev_script.clone());
        }
        if payload.dev_script_working_dir.is_some() {
            active.dev_script_working_dir = Set(payload.dev_script_working_dir.clone());
        }
        if payload.default_agent_working_dir.is_some() {
            active.default_agent_working_dir = Set(payload.default_agent_working_dir.clone());
        }
        if let Some(git_no_verify_override) = payload.git_no_verify_override {
            active.git_no_verify_override = Set(git_no_verify_override);
        }
        if let Some(scheduler_max_concurrent) = payload.scheduler_max_concurrent {
            active.scheduler_max_concurrent = Set(std::cmp::max(scheduler_max_concurrent, 1));
        }
        if let Some(scheduler_max_retries) = payload.scheduler_max_retries {
            active.scheduler_max_retries = Set(std::cmp::max(scheduler_max_retries, 0));
        }
        if let Some(default_continuation_turns) = payload.default_continuation_turns {
            active.default_continuation_turns = Set(std::cmp::max(default_continuation_turns, 0));
        }
        if let Some(hook) = payload.after_prepare_hook.clone() {
            apply_hook_update(&mut active, hook, true);
        }
        if let Some(hook) = payload.before_cleanup_hook.clone() {
            apply_hook_update(&mut active, hook, false);
        }
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;
        let payload = serde_json::to_value(ProjectEventPayload { project_id: id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_PROJECT_UPDATED, "project", id, payload).await?;
        Ok(Self::from_model(updated))
    }

    pub async fn clear_default_agent_working_dir<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<(), DbErr> {
        let record = project::Entity::find()
            .filter(project::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let mut active: project::ActiveModel = record.into();
        active.default_agent_working_dir = Set(Some(String::new()));
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let payload = serde_json::to_value(ProjectEventPayload { project_id: id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_PROJECT_UPDATED, "project", id, payload).await?;
        Ok(())
    }

    pub async fn set_remote_project_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        remote_project_id: Option<Uuid>,
    ) -> Result<(), DbErr> {
        let record = project::Entity::find()
            .filter(project::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound("Project not found".to_string()))?;
        let mut active: project::ActiveModel = record.into();
        active.remote_project_id = Set(remote_project_id);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
        let payload = serde_json::to_value(ProjectEventPayload { project_id: id })
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        EventOutbox::enqueue(db, EVENT_PROJECT_UPDATED, "project", id, payload).await?;
        Ok(())
    }

    pub async fn set_remote_project_id_tx<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        remote_project_id: Option<Uuid>,
    ) -> Result<(), DbErr> {
        Self::set_remote_project_id(db, id, remote_project_id).await
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, DbErr> {
        let project = project::Entity::find()
            .filter(project::Column::Uuid.eq(id))
            .one(db)
            .await?;

        let Some(project) = project else {
            return Ok(0);
        };

        let tasks = task::Entity::find()
            .filter(task::Column::ProjectId.eq(project.id))
            .all(db)
            .await?;

        let result = project::Entity::delete_many()
            .filter(project::Column::Uuid.eq(id))
            .exec(db)
            .await?;

        if result.rows_affected > 0 {
            for task_model in tasks {
                let payload = serde_json::to_value(TaskEventPayload {
                    task_id: task_model.uuid,
                    project_id: id,
                })
                .map_err(|err| DbErr::Custom(err.to_string()))?;
                EventOutbox::enqueue(db, EVENT_TASK_DELETED, "task", task_model.uuid, payload)
                    .await?;
            }

            let payload = serde_json::to_value(ProjectEventPayload { project_id: id })
                .map_err(|err| DbErr::Custom(err.to_string()))?;
            EventOutbox::enqueue(db, EVENT_PROJECT_DELETED, "project", id, payload).await?;
        }

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use uuid::Uuid;

    use super::Project;

    #[test]
    fn effective_git_no_verify_prefers_project_override() {
        let now = Utc::now();

        let project = Project {
            id: Uuid::new_v4(),
            name: "p".to_string(),
            dev_script: None,
            dev_script_working_dir: None,
            default_agent_working_dir: None,
            git_no_verify_override: Some(false),
            scheduler_max_concurrent: 1,
            scheduler_max_retries: 3,
            default_continuation_turns: 0,
            after_prepare_hook: None,
            before_cleanup_hook: None,
            remote_project_id: None,
            created_at: now,
            updated_at: now,
        };

        assert!(!project.effective_git_no_verify(true));

        let inherited = Project {
            git_no_verify_override: None,
            ..project
        };

        assert!(inherited.effective_git_no_verify(true));
    }
}
