use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use executors_protocol::ExecutorProfileId;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use utils_core::text::milestone_integration_branch_name;
use uuid::Uuid;

use crate::{
    entities::{milestone, task},
    events::{EVENT_TASK_UPDATED, TaskEventPayload},
    models::{event_outbox::EventOutbox, ids, milestone_plan_application::MilestonePlanApplicationSummary},
    types::{MilestoneAutomationMode, TaskKind, TaskStatus},
};

const SUPPORTED_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Error)]
pub enum MilestoneError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("Milestone not found")]
    MilestoneNotFound,
    #[error("Project not found")]
    ProjectNotFound,
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    #[error("Task belongs to another project: {0}")]
    TaskProjectMismatch(String),
    #[error("Task already linked to another milestone: {0}")]
    MilestoneMismatch(String),
    #[error("Task kind 'milestone' cannot be used for milestone nodes: {0}")]
    TaskKindMismatch(String),
    #[error("Unsupported schema version: {0}")]
    UnsupportedSchemaVersion(i32),
    #[error("Invalid milestone graph: {0}")]
    InvalidGraph(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Milestone {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub objective: Option<String>,
    pub definition_of_done: Option<String>,
    pub default_executor_profile_id: Option<ExecutorProfileId>,
    pub automation_mode: MilestoneAutomationMode,
    pub run_next_step_requested_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,
    pub baseline_ref: String,
    pub schema_version: i32,
    pub graph: MilestoneGraph,
    pub suggested_status: TaskStatus,
    #[serde(default)]
    pub last_plan_application: Option<MilestonePlanApplicationSummary>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateMilestone {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub objective: Option<String>,
    pub definition_of_done: Option<String>,
    pub default_executor_profile_id: Option<ExecutorProfileId>,
    pub automation_mode: Option<MilestoneAutomationMode>,
    pub status: Option<TaskStatus>,
    pub baseline_ref: Option<String>,
    pub schema_version: i32,
    pub graph: MilestoneGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateMilestone {
    pub title: Option<String>,
    pub description: Option<String>,
    pub objective: Option<String>,
    pub definition_of_done: Option<String>,
    pub default_executor_profile_id: Option<Option<ExecutorProfileId>>,
    pub automation_mode: Option<MilestoneAutomationMode>,
    pub status: Option<TaskStatus>,
    pub baseline_ref: Option<String>,
    pub schema_version: Option<i32>,
    pub graph: Option<MilestoneGraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestoneGraph {
    pub nodes: Vec<MilestoneNode>,
    #[serde(default)]
    pub edges: Vec<MilestoneEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestoneNode {
    pub id: String,
    pub task_id: Uuid,
    #[serde(default)]
    pub kind: MilestoneNodeKind,
    pub phase: i32,
    #[serde(default)]
    pub executor_profile_id: Option<ExecutorProfileId>,
    #[serde(default)]
    pub base_strategy: MilestoneNodeBaseStrategy,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    pub layout: MilestoneNodeLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestoneNodeLayout {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum MilestoneNodeKind {
    #[default]
    Task,
    Checkpoint,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum MilestoneNodeBaseStrategy {
    #[default]
    Topology,
    Baseline,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct MilestoneEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub data_flow: Option<String>,
}

impl MilestoneGraph {
    fn without_statuses(&self) -> Self {
        let mut graph = self.clone();
        for node in &mut graph.nodes {
            node.status = None;
            if node
                .instructions
                .as_ref()
                .is_some_and(|instructions| instructions.trim().is_empty())
            {
                node.instructions = None;
            }
        }
        graph
    }
}

fn validate_schema_version(schema_version: i32) -> Result<(), MilestoneError> {
    if schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(MilestoneError::UnsupportedSchemaVersion(schema_version));
    }
    Ok(())
}

fn validate_graph(graph: &MilestoneGraph) -> Result<(), MilestoneError> {
    let mut node_ids = HashSet::new();
    let mut task_ids = HashSet::new();
    for node in &graph.nodes {
        let trimmed = node.id.trim();
        if trimmed.is_empty() {
            return Err(MilestoneError::InvalidGraph(
                "node id cannot be empty".to_string(),
            ));
        }
        if !node_ids.insert(trimmed.to_string()) {
            return Err(MilestoneError::InvalidGraph(format!(
                "duplicate node id: {}",
                trimmed
            )));
        }
        if !task_ids.insert(node.task_id) {
            return Err(MilestoneError::InvalidGraph(format!(
                "duplicate task id in nodes: {}",
                node.task_id
            )));
        }
    }

    let mut incoming: HashMap<String, usize> = HashMap::new();
    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
    for node_id in node_ids.iter() {
        incoming.insert(node_id.clone(), 0);
        outgoing.insert(node_id.clone(), Vec::new());
    }

    for edge in &graph.edges {
        let from = edge.from.trim();
        let to = edge.to.trim();
        if from.is_empty() || to.is_empty() {
            return Err(MilestoneError::InvalidGraph(
                "edge endpoints cannot be empty".to_string(),
            ));
        }
        if from == to {
            return Err(MilestoneError::InvalidGraph(format!(
                "self edge is not allowed: {}",
                from
            )));
        }
        if !node_ids.contains(from) || !node_ids.contains(to) {
            return Err(MilestoneError::InvalidGraph(format!(
                "edge references missing node: {} -> {}",
                from, to
            )));
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
    let mut visited = 0usize;

    while let Some(node_id) = queue.pop_front() {
        visited += 1;
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

    if visited != node_ids.len() {
        return Err(MilestoneError::InvalidGraph(
            "graph contains a cycle".to_string(),
        ));
    }

    Ok(())
}

fn aggregate_status(statuses: &[TaskStatus]) -> TaskStatus {
    if statuses.contains(&TaskStatus::InReview) {
        return TaskStatus::InReview;
    }
    if statuses.contains(&TaskStatus::InProgress) {
        return TaskStatus::InProgress;
    }
    if !statuses.is_empty() && statuses.iter().all(|status| *status == TaskStatus::Done) {
        return TaskStatus::Done;
    }
    if !statuses.is_empty()
        && statuses
            .iter()
            .all(|status| *status == TaskStatus::Cancelled)
    {
        return TaskStatus::Cancelled;
    }
    TaskStatus::Todo
}

impl Milestone {
    fn parse_graph(graph_json: serde_json::Value) -> Result<MilestoneGraph, MilestoneError> {
        Ok(serde_json::from_value(graph_json)?)
    }

    async fn build_node_status_map<C: ConnectionTrait>(
        db: &C,
        nodes: &[MilestoneNode],
    ) -> Result<HashMap<Uuid, TaskStatus>, MilestoneError> {
        if nodes.is_empty() {
            return Ok(HashMap::new());
        }
        let task_ids: Vec<Uuid> = nodes.iter().map(|node| node.task_id).collect();
        let task_models = task::Entity::find()
            .filter(task::Column::Uuid.is_in(task_ids.clone()))
            .all(db)
            .await?;

        let mut status_map = HashMap::new();
        for model in task_models {
            status_map.insert(model.uuid, model.status);
        }
        Ok(status_map)
    }

    fn apply_node_statuses(
        mut graph: MilestoneGraph,
        status_map: &HashMap<Uuid, TaskStatus>,
    ) -> (MilestoneGraph, Vec<TaskStatus>) {
        let mut statuses = Vec::with_capacity(graph.nodes.len());
        for node in &mut graph.nodes {
            let status = status_map
                .get(&node.task_id)
                .cloned()
                .unwrap_or(TaskStatus::Todo);
            node.status = Some(status.clone());
            statuses.push(status);
        }
        (graph, statuses)
    }

    async fn from_model<C: ConnectionTrait>(
        db: &C,
        model: milestone::Model,
        last_plan_application: Option<MilestonePlanApplicationSummary>,
    ) -> Result<Self, MilestoneError> {
        let project_uuid = ids::project_uuid_by_id(db, model.project_id)
            .await?
            .ok_or(MilestoneError::ProjectNotFound)?;
        let default_executor_profile_id = match model.default_executor_profile_id {
            Some(value) => Some(serde_json::from_value(value)?),
            None => None,
        };
        let graph = Self::parse_graph(model.graph_json)?;
        let status_map = Self::build_node_status_map(db, &graph.nodes).await?;
        let (graph_with_status, node_statuses) = Self::apply_node_statuses(graph, &status_map);
        let suggested_status = aggregate_status(&node_statuses);

        Ok(Self {
            id: model.uuid,
            project_id: project_uuid,
            title: model.title,
            description: model.description,
            objective: model.objective,
            definition_of_done: model.definition_of_done,
            default_executor_profile_id,
            automation_mode: model.automation_mode,
            run_next_step_requested_at: model.run_next_step_requested_at.map(Into::into),
            status: model.status,
            baseline_ref: model.baseline_ref,
            schema_version: model.schema_version,
            graph: graph_with_status,
            suggested_status,
            last_plan_application,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        })
    }

    pub async fn find_by_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<Self>, MilestoneError> {
        let record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => {
                let last_plan_application =
                    crate::models::milestone_plan_application::find_latest_by_milestone_row_id(
                        db,
                        model.id,
                        model.uuid,
                    )
                    .await?;
                Ok(Some(
                    Self::from_model(db, model, last_plan_application).await?,
                ))
            }
            None => Ok(None),
        }
    }

    pub async fn find_by_project_id<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<Self>, MilestoneError> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(MilestoneError::ProjectNotFound)?;

        let models = milestone::Entity::find()
            .filter(milestone::Column::ProjectId.eq(project_row_id))
            .order_by_desc(milestone::Column::CreatedAt)
            .all(db)
            .await?;

        let row_ids: Vec<i64> = models.iter().map(|model| model.id).collect();
        let last_plan_applications = crate::models::milestone_plan_application::find_latest_by_milestone_row_ids(db, &row_ids)
            .await?;

        let mut groups = Vec::with_capacity(models.len());
        for model in models {
            let last_plan_application = last_plan_applications.get(&model.id).cloned();
            groups.push(Self::from_model(db, model, last_plan_application).await?);
        }
        Ok(groups)
    }

    pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, MilestoneError> {
        let models = milestone::Entity::find()
            .order_by_desc(milestone::Column::CreatedAt)
            .all(db)
            .await?;

        let row_ids: Vec<i64> = models.iter().map(|model| model.id).collect();
        let last_plan_applications = crate::models::milestone_plan_application::find_latest_by_milestone_row_ids(db, &row_ids)
            .await?;

        let mut groups = Vec::with_capacity(models.len());
        for model in models {
            let last_plan_application = last_plan_applications.get(&model.id).cloned();
            groups.push(Self::from_model(db, model, last_plan_application).await?);
        }
        Ok(groups)
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateMilestone,
        milestone_id: Uuid,
    ) -> Result<Self, MilestoneError> {
        validate_schema_version(data.schema_version)?;
        validate_graph(&data.graph)?;

        let project_row_id = ids::project_id_by_uuid(db, data.project_id)
            .await?
            .ok_or(MilestoneError::ProjectNotFound)?;

        let graph_json = serde_json::to_value(data.graph.without_statuses())?;
        let objective = data
            .objective
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let definition_of_done = data
            .definition_of_done
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let default_executor_profile_id = match data.default_executor_profile_id.as_ref() {
            Some(profile) => Some(serde_json::to_value(profile)?),
            None => None,
        };
        let now = Utc::now();
        let baseline_ref = data
            .baseline_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| milestone_integration_branch_name(&milestone_id));
        let active = milestone::ActiveModel {
            uuid: Set(milestone_id),
            project_id: Set(project_row_id),
            title: Set(data.title.clone()),
            description: Set(data.description.clone()),
            objective: Set(objective),
            definition_of_done: Set(definition_of_done),
            default_executor_profile_id: Set(default_executor_profile_id),
            automation_mode: Set(data.automation_mode.clone().unwrap_or_default()),
            run_next_step_requested_at: Set(None),
            status: Set(data.status.clone().unwrap_or_default()),
            baseline_ref: Set(baseline_ref),
            schema_version: Set(data.schema_version),
            graph_json: Set(graph_json),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;

        let entry_task = crate::models::task::CreateTask {
            project_id: data.project_id,
            title: data.title.clone(),
            description: data
                .description
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            status: None,
            task_kind: Some(TaskKind::Milestone),
            milestone_id: Some(milestone_id),
            milestone_node_id: None,
            parent_workspace_id: None,
            origin_task_id: None,
            created_by_kind: None,
            image_ids: None,
            shared_task_id: None,
        };
        let _ = crate::models::task::Task::create(db, &entry_task, Uuid::new_v4()).await?;

        Self::sync_task_links(db, model.id, project_row_id, &data.graph).await?;
        Self::sync_entry_task_statuses_by_row_id(db, model.id).await?;

        Self::from_model(db, model, None).await
    }

    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        data: &UpdateMilestone,
    ) -> Result<Self, MilestoneError> {
        let record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(MilestoneError::MilestoneNotFound)?;

        let status_changed = data
            .status
            .as_ref()
            .map(|value| value != &record.status)
            .unwrap_or(false);
        if let Some(schema_version) = data.schema_version {
            validate_schema_version(schema_version)?;
        }

        let mut title = record.title.clone();
        let mut description = record.description.clone();
        let mut objective = record.objective.clone();
        let mut definition_of_done = record.definition_of_done.clone();
        let mut default_executor_profile_id = record.default_executor_profile_id.clone();
        let mut automation_mode = record.automation_mode.clone();
        let mut status = record.status.clone();
        let mut baseline_ref = record.baseline_ref.clone();
        let mut schema_version = record.schema_version;
        let mut graph = Self::parse_graph(record.graph_json.clone())?;

        if let Some(value) = &data.title {
            title = value.clone();
        }
        if let Some(value) = &data.description {
            let trimmed = value.trim();
            description = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        if let Some(value) = &data.objective {
            let trimmed = value.trim();
            objective = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        if let Some(value) = &data.definition_of_done {
            let trimmed = value.trim();
            definition_of_done = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
        }
        if let Some(value) = &data.default_executor_profile_id {
            default_executor_profile_id = match value {
                Some(profile) => Some(serde_json::to_value(profile)?),
                None => None,
            };
        }
        if let Some(value) = &data.automation_mode {
            automation_mode = value.clone();
        }
        if let Some(value) = data.status.clone() {
            status = value;
        }
        if let Some(value) = &data.baseline_ref {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                baseline_ref = trimmed.to_string();
            }
        }
        if let Some(value) = data.schema_version {
            schema_version = value;
        }
        if let Some(value) = &data.graph {
            validate_graph(value)?;
            graph = value.clone();
        }

        let mut active: milestone::ActiveModel = record.into();
        active.title = Set(title);
        active.description = Set(description);
        active.objective = Set(objective);
        active.definition_of_done = Set(definition_of_done);
        active.default_executor_profile_id = Set(default_executor_profile_id);
        active.automation_mode = Set(automation_mode);
        active.status = Set(status);
        active.baseline_ref = Set(baseline_ref);
        active.schema_version = Set(schema_version);
        active.graph_json = Set(serde_json::to_value(graph.without_statuses())?);
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;

        if let Some(graph) = &data.graph {
            Self::sync_task_links(db, updated.id, updated.project_id, graph).await?;
        }
        if data.graph.is_some() || status_changed {
            Self::sync_entry_task_statuses_by_row_id(db, updated.id).await?;
        }

        let last_plan_application =
            crate::models::milestone_plan_application::find_latest_by_milestone_row_id(
                db,
                updated.id,
                updated.uuid,
            )
            .await?;
        Self::from_model(db, updated, last_plan_application).await
    }

    pub async fn request_run_next_step<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<DateTime<Utc>, MilestoneError> {
        let record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(MilestoneError::MilestoneNotFound)?;

        let now = Utc::now();
        let mut active: milestone::ActiveModel = record.into();
        active.run_next_step_requested_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(db).await?;

        Ok(now)
    }

    pub async fn clear_run_next_step_request<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<(), MilestoneError> {
        let record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(MilestoneError::MilestoneNotFound)?;

        let now = Utc::now();
        let mut active: milestone::ActiveModel = record.into();
        active.run_next_step_requested_at = Set(None);
        active.updated_at = Set(now.into());
        active.update(db).await?;

        Ok(())
    }

    pub async fn delete<C: ConnectionTrait>(db: &C, id: Uuid) -> Result<u64, MilestoneError> {
        let record = milestone::Entity::find()
            .filter(milestone::Column::Uuid.eq(id))
            .one(db)
            .await?;

        let Some(record) = record else {
            return Ok(0);
        };

        let milestone_row_id = record.id;
        let tasks = task::Entity::find()
            .filter(task::Column::MilestoneId.eq(milestone_row_id))
            .all(db)
            .await?;

        for task_model in tasks {
            Self::clear_task_link(db, &task_model).await?;
        }

        let result = milestone::Entity::delete_many()
            .filter(milestone::Column::Uuid.eq(id))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    pub async fn sync_entry_task_statuses_by_row_id<C: ConnectionTrait>(
        db: &C,
        milestone_row_id: i64,
    ) -> Result<TaskStatus, MilestoneError> {
        let record = milestone::Entity::find_by_id(milestone_row_id)
            .one(db)
            .await?
            .ok_or(MilestoneError::MilestoneNotFound)?;

        let graph = Self::parse_graph(record.graph_json)?;
        let status_map = Self::build_node_status_map(db, &graph.nodes).await?;
        let (_, node_statuses) = Self::apply_node_statuses(graph, &status_map);
        let suggested_status = aggregate_status(&node_statuses);
        let entry_status = record.status.clone();

        let entry_tasks = task::Entity::find()
            .filter(task::Column::MilestoneId.eq(milestone_row_id))
            .filter(task::Column::TaskKind.eq(TaskKind::Milestone))
            .all(db)
            .await?;

        for task_model in entry_tasks {
            if task_model.status != entry_status {
                let mut active: task::ActiveModel = task_model.clone().into();
                active.status = Set(entry_status.clone());
                active.updated_at = Set(Utc::now().into());
                let updated = active.update(db).await?;
                Self::enqueue_task_updated(db, &updated).await?;
            }
        }

        Ok(suggested_status)
    }

    async fn sync_task_links<C: ConnectionTrait>(
        db: &C,
        milestone_row_id: i64,
        project_row_id: i64,
        graph: &MilestoneGraph,
    ) -> Result<(), MilestoneError> {
        let task_ids: Vec<Uuid> = graph.nodes.iter().map(|node| node.task_id).collect();
        if !task_ids.is_empty() {
            let task_models = task::Entity::find()
                .filter(task::Column::Uuid.is_in(task_ids.clone()))
                .all(db)
                .await?;
            let task_map: HashMap<Uuid, task::Model> = task_models
                .into_iter()
                .map(|model| (model.uuid, model))
                .collect();

            for task_id in &task_ids {
                if !task_map.contains_key(task_id) {
                    return Err(MilestoneError::TaskNotFound(task_id.to_string()));
                }
            }

            for node in &graph.nodes {
                let task_model = task_map
                    .get(&node.task_id)
                    .ok_or_else(|| MilestoneError::TaskNotFound(node.task_id.to_string()))?;

                if task_model.project_id != project_row_id {
                    return Err(MilestoneError::TaskProjectMismatch(
                        node.task_id.to_string(),
                    ));
                }
                if task_model.milestone_id.is_some()
                    && task_model.milestone_id != Some(milestone_row_id)
                {
                    return Err(MilestoneError::MilestoneMismatch(node.task_id.to_string()));
                }
                if task_model.task_kind == TaskKind::Milestone {
                    return Err(MilestoneError::TaskKindMismatch(node.task_id.to_string()));
                }
            }

            let mut node_id_map: HashMap<Uuid, String> = HashMap::new();
            for node in &graph.nodes {
                node_id_map.insert(node.task_id, node.id.trim().to_string());
            }

            for (task_id, node_id) in &node_id_map {
                let task_model = task_map
                    .get(task_id)
                    .ok_or_else(|| MilestoneError::TaskNotFound(task_id.to_string()))?;
                let mut active: task::ActiveModel = task_model.clone().into();
                active.milestone_id = Set(Some(milestone_row_id));
                active.milestone_node_id = Set(Some(node_id.clone()));
                active.updated_at = Set(Utc::now().into());
                let updated = active.update(db).await?;
                Self::enqueue_task_updated(db, &updated).await?;
            }
        }

        let current_group_tasks = task::Entity::find()
            .filter(task::Column::MilestoneId.eq(milestone_row_id))
            .filter(task::Column::TaskKind.ne(TaskKind::Milestone))
            .all(db)
            .await?;

        for task_model in current_group_tasks {
            if !task_ids.contains(&task_model.uuid) {
                Self::clear_task_link(db, &task_model).await?;
            }
        }

        Ok(())
    }

    async fn clear_task_link<C: ConnectionTrait>(
        db: &C,
        task_model: &task::Model,
    ) -> Result<(), MilestoneError> {
        let mut active: task::ActiveModel = task_model.clone().into();
        active.milestone_id = Set(None);
        active.milestone_node_id = Set(None);
        active.updated_at = Set(Utc::now().into());
        let updated = active.update(db).await?;
        Self::enqueue_task_updated(db, &updated).await?;
        Ok(())
    }

    async fn enqueue_task_updated<C: ConnectionTrait>(
        db: &C,
        task_model: &task::Model,
    ) -> Result<(), MilestoneError> {
        let project_uuid = ids::project_uuid_by_id(db, task_model.project_id)
            .await?
            .ok_or(MilestoneError::ProjectNotFound)?;
        let payload = serde_json::to_value(TaskEventPayload {
            task_id: task_model.uuid,
            project_id: project_uuid,
        })?;
        EventOutbox::enqueue(db, EVENT_TASK_UPDATED, "task", task_model.uuid, payload).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::MigratorTrait;

    use super::*;
    use crate::models::{
        project::{CreateProject, Project},
        task::{CreateTask, Task, TaskKind},
    };

    fn base_graph() -> MilestoneGraph {
        MilestoneGraph {
            nodes: vec![
                MilestoneNode {
                    id: "node-a".to_string(),
                    task_id: Uuid::new_v4(),
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                    status: None,
                },
                MilestoneNode {
                    id: "node-b".to_string(),
                    task_id: Uuid::new_v4(),
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 1.0, y: 1.0 },
                    status: None,
                },
            ],
            edges: vec![MilestoneEdge {
                id: "edge-a-b".to_string(),
                from: "node-a".to_string(),
                to: "node-b".to_string(),
                data_flow: None,
            }],
        }
    }

    #[test]
    fn validate_graph_accepts_dag() {
        let graph = base_graph();
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn validate_graph_rejects_duplicate_node_ids() {
        let mut graph = base_graph();
        graph.nodes[1].id = graph.nodes[0].id.clone();
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, MilestoneError::InvalidGraph(_)));
    }

    #[test]
    fn validate_graph_rejects_self_edges() {
        let mut graph = base_graph();
        graph.edges[0].from = "node-a".to_string();
        graph.edges[0].to = "node-a".to_string();
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, MilestoneError::InvalidGraph(_)));
    }

    #[test]
    fn validate_graph_rejects_missing_nodes() {
        let mut graph = base_graph();
        graph.edges[0].to = "missing".to_string();
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, MilestoneError::InvalidGraph(_)));
    }

    #[test]
    fn validate_graph_rejects_cycles() {
        let mut graph = base_graph();
        graph.edges.push(MilestoneEdge {
            id: "edge-b-a".to_string(),
            from: "node-b".to_string(),
            to: "node-a".to_string(),
            data_flow: None,
        });
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, MilestoneError::InvalidGraph(_)));
    }

    #[test]
    fn aggregate_status_respects_priority() {
        let statuses = vec![TaskStatus::Done, TaskStatus::InReview];
        assert_eq!(aggregate_status(&statuses), TaskStatus::InReview);

        let statuses = vec![TaskStatus::InProgress, TaskStatus::Todo];
        assert_eq!(aggregate_status(&statuses), TaskStatus::InProgress);
    }

    #[test]
    fn aggregate_status_handles_terminal_states() {
        let statuses = vec![TaskStatus::Done, TaskStatus::Done];
        assert_eq!(aggregate_status(&statuses), TaskStatus::Done);

        let statuses = vec![TaskStatus::Cancelled, TaskStatus::Cancelled];
        assert_eq!(aggregate_status(&statuses), TaskStatus::Cancelled);

        let statuses = vec![TaskStatus::Done, TaskStatus::Cancelled];
        assert_eq!(aggregate_status(&statuses), TaskStatus::Todo);
    }

    #[test]
    fn aggregate_status_defaults_to_todo() {
        let statuses = Vec::new();
        assert_eq!(aggregate_status(&statuses), TaskStatus::Todo);
    }

    async fn setup_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db_migration::Migrator::up(&db, None).await.unwrap();
        db
    }

    #[tokio::test]
    async fn create_milestone_creates_entry_task() {
        let db = setup_db().await;
        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Milestone project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_a_id = Uuid::new_v4();
        let task_b_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Task A".to_string(), None),
            task_a_id,
        )
        .await
        .unwrap();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Task B".to_string(), None),
            task_b_id,
        )
        .await
        .unwrap();

        let graph = MilestoneGraph {
            nodes: vec![
                MilestoneNode {
                    id: "node-a".to_string(),
                    task_id: task_a_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                    status: None,
                },
                MilestoneNode {
                    id: "node-b".to_string(),
                    task_id: task_b_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 1.0, y: 1.0 },
                    status: None,
                },
            ],
            edges: Vec::new(),
        };

        let milestone_id = Uuid::new_v4();
        let created = Milestone::create(
            &db,
            &CreateMilestone {
                project_id,
                title: "Workflow".to_string(),
                description: None,
                objective: None,
                definition_of_done: None,
                default_executor_profile_id: None,
                automation_mode: None,
                status: None,
                baseline_ref: Some("main".to_string()),
                schema_version: SUPPORTED_SCHEMA_VERSION,
                graph,
            },
            milestone_id,
        )
        .await
        .unwrap();

        let tasks = Task::find_by_milestone_id(&db, created.id).await.unwrap();
        let entry_tasks: Vec<_> = tasks
            .iter()
            .filter(|task| task.task_kind == TaskKind::Milestone)
            .collect();
        assert_eq!(entry_tasks.len(), 1);
        assert_eq!(entry_tasks[0].milestone_id, Some(created.id));

        let node_tasks: Vec<_> = tasks
            .iter()
            .filter(|task| task.task_kind != TaskKind::Milestone)
            .collect();
        assert_eq!(node_tasks.len(), 2);
        assert!(
            node_tasks
                .iter()
                .all(|task| task.milestone_node_id.is_some())
        );
    }

    #[tokio::test]
    async fn instructions_persist_and_blank_normalizes_to_none() {
        let db = setup_db().await;
        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Instruction project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_a_id = Uuid::new_v4();
        let task_b_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Task A".to_string(), None),
            task_a_id,
        )
        .await
        .unwrap();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Task B".to_string(), None),
            task_b_id,
        )
        .await
        .unwrap();

        let graph = MilestoneGraph {
            nodes: vec![
                MilestoneNode {
                    id: "node-a".to_string(),
                    task_id: task_a_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: Some("Do the thing".to_string()),
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                    status: None,
                },
                MilestoneNode {
                    id: "node-b".to_string(),
                    task_id: task_b_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: Some("   ".to_string()),
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 1.0, y: 1.0 },
                    status: None,
                },
            ],
            edges: Vec::new(),
        };

        let milestone_id = Uuid::new_v4();
        let created = Milestone::create(
            &db,
            &CreateMilestone {
                project_id,
                title: "Workflow".to_string(),
                description: None,
                objective: None,
                definition_of_done: None,
                default_executor_profile_id: None,
                automation_mode: None,
                status: None,
                baseline_ref: Some("main".to_string()),
                schema_version: SUPPORTED_SCHEMA_VERSION,
                graph,
            },
            milestone_id,
        )
        .await
        .unwrap();

        let created_node_a = created
            .graph
            .nodes
            .iter()
            .find(|node| node.id == "node-a")
            .expect("node-a");
        assert_eq!(created_node_a.instructions.as_deref(), Some("Do the thing"));

        let created_node_b = created
            .graph
            .nodes
            .iter()
            .find(|node| node.id == "node-b")
            .expect("node-b");
        assert!(created_node_b.instructions.is_none());

        let updated_graph = MilestoneGraph {
            nodes: vec![
                MilestoneNode {
                    id: "node-a".to_string(),
                    task_id: task_a_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: Some("Updated instructions".to_string()),
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                    status: None,
                },
                MilestoneNode {
                    id: "node-b".to_string(),
                    task_id: task_b_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: Some("\n\t".to_string()),
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 1.0, y: 1.0 },
                    status: None,
                },
            ],
            edges: Vec::new(),
        };

        let updated = Milestone::update(
            &db,
            milestone_id,
            &UpdateMilestone {
                title: None,
                description: None,
                objective: None,
                definition_of_done: None,
                default_executor_profile_id: None,
                automation_mode: None,
                status: None,
                baseline_ref: None,
                schema_version: None,
                graph: Some(updated_graph),
            },
        )
        .await
        .unwrap();

        let updated_node_a = updated
            .graph
            .nodes
            .iter()
            .find(|node| node.id == "node-a")
            .expect("node-a updated");
        assert_eq!(
            updated_node_a.instructions.as_deref(),
            Some("Updated instructions")
        );

        let updated_node_b = updated
            .graph
            .nodes
            .iter()
            .find(|node| node.id == "node-b")
            .expect("node-b updated");
        assert!(updated_node_b.instructions.is_none());
    }

    #[tokio::test]
    async fn entry_task_status_reflects_milestone_status() {
        let db = setup_db().await;
        let project_id = Uuid::new_v4();
        Project::create(
            &db,
            &CreateProject {
                name: "Status project".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_a_id = Uuid::new_v4();
        let task_b_id = Uuid::new_v4();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Task A".to_string(), None),
            task_a_id,
        )
        .await
        .unwrap();
        Task::create(
            &db,
            &CreateTask::from_title_description(project_id, "Task B".to_string(), None),
            task_b_id,
        )
        .await
        .unwrap();

        let graph = MilestoneGraph {
            nodes: vec![
                MilestoneNode {
                    id: "node-a".to_string(),
                    task_id: task_a_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 0.0, y: 0.0 },
                    status: None,
                },
                MilestoneNode {
                    id: "node-b".to_string(),
                    task_id: task_b_id,
                    kind: MilestoneNodeKind::Task,
                    phase: 0,
                    executor_profile_id: None,
                    base_strategy: MilestoneNodeBaseStrategy::Topology,
                    instructions: None,
                    requires_approval: None,
                    layout: MilestoneNodeLayout { x: 1.0, y: 1.0 },
                    status: None,
                },
            ],
            edges: Vec::new(),
        };

        let milestone_id = Uuid::new_v4();
        Milestone::create(
            &db,
            &CreateMilestone {
                project_id,
                title: "Workflow".to_string(),
                description: None,
                objective: None,
                definition_of_done: None,
                default_executor_profile_id: None,
                automation_mode: None,
                status: None,
                baseline_ref: Some("main".to_string()),
                schema_version: SUPPORTED_SCHEMA_VERSION,
                graph,
            },
            milestone_id,
        )
        .await
        .unwrap();

        let mut tasks = Task::find_by_milestone_id(&db, milestone_id).await.unwrap();
        let entry = tasks
            .iter()
            .find(|task| task.task_kind == TaskKind::Milestone)
            .expect("entry task");
        assert_eq!(entry.status, TaskStatus::Todo);

        let node_task = tasks
            .iter()
            .find(|task| task.task_kind != TaskKind::Milestone)
            .expect("node task");
        Task::update_status(&db, node_task.id, TaskStatus::InProgress)
            .await
            .unwrap();

        tasks = Task::find_by_milestone_id(&db, milestone_id).await.unwrap();
        let updated_entry = tasks
            .iter()
            .find(|task| task.task_kind == TaskKind::Milestone)
            .expect("updated entry task");
        assert_eq!(updated_entry.status, TaskStatus::Todo);

        let updated_milestone = Milestone::find_by_id(&db, milestone_id)
            .await
            .unwrap()
            .expect("updated milestone");
        assert_eq!(updated_milestone.status, TaskStatus::Todo);
        assert_eq!(updated_milestone.suggested_status, TaskStatus::InProgress);
    }
}
