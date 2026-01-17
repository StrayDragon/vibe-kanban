use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use crate::{
    entities::{task, task_group},
    events::{EVENT_TASK_UPDATED, TaskEventPayload},
    models::{event_outbox::EventOutbox, ids},
    types::{TaskKind, TaskStatus},
};

const SUPPORTED_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Error)]
pub enum TaskGroupError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("Task group not found")]
    TaskGroupNotFound,
    #[error("Project not found")]
    ProjectNotFound,
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    #[error("Task belongs to another project: {0}")]
    TaskProjectMismatch(String),
    #[error("Task already linked to another task group: {0}")]
    TaskGroupMismatch(String),
    #[error("Task kind 'group' cannot be used for task group nodes: {0}")]
    TaskKindMismatch(String),
    #[error("Unsupported schema version: {0}")]
    UnsupportedSchemaVersion(i32),
    #[error("Invalid task group graph: {0}")]
    InvalidGraph(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskGroup {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub baseline_ref: String,
    pub schema_version: i32,
    pub graph: TaskGroupGraph,
    pub suggested_status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateTaskGroup {
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub baseline_ref: String,
    pub schema_version: i32,
    pub graph: TaskGroupGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateTaskGroup {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub baseline_ref: Option<String>,
    pub schema_version: Option<i32>,
    pub graph: Option<TaskGroupGraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskGroupGraph {
    pub nodes: Vec<TaskGroupNode>,
    #[serde(default)]
    pub edges: Vec<TaskGroupEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskGroupNode {
    pub id: String,
    pub task_id: Uuid,
    #[serde(default)]
    pub kind: TaskGroupNodeKind,
    pub phase: i32,
    #[serde(default)]
    pub agent_role: Option<String>,
    #[serde(default)]
    pub cost_estimate: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    pub layout: TaskGroupNodeLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskGroupNodeLayout {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(use_ts_enum)]
pub enum TaskGroupNodeKind {
    #[default]
    Task,
    Checkpoint,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskGroupEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub data_flow: Option<String>,
}

impl TaskGroupGraph {
    fn without_statuses(&self) -> Self {
        let mut graph = self.clone();
        for node in &mut graph.nodes {
            node.status = None;
        }
        graph
    }
}

fn validate_schema_version(schema_version: i32) -> Result<(), TaskGroupError> {
    if schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(TaskGroupError::UnsupportedSchemaVersion(schema_version));
    }
    Ok(())
}

fn validate_graph(graph: &TaskGroupGraph) -> Result<(), TaskGroupError> {
    let mut node_ids = HashSet::new();
    let mut task_ids = HashSet::new();
    for node in &graph.nodes {
        let trimmed = node.id.trim();
        if trimmed.is_empty() {
            return Err(TaskGroupError::InvalidGraph(
                "node id cannot be empty".to_string(),
            ));
        }
        if !node_ids.insert(trimmed.to_string()) {
            return Err(TaskGroupError::InvalidGraph(format!(
                "duplicate node id: {}",
                trimmed
            )));
        }
        if !task_ids.insert(node.task_id) {
            return Err(TaskGroupError::InvalidGraph(format!(
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
            return Err(TaskGroupError::InvalidGraph(
                "edge endpoints cannot be empty".to_string(),
            ));
        }
        if from == to {
            return Err(TaskGroupError::InvalidGraph(format!(
                "self edge is not allowed: {}",
                from
            )));
        }
        if !node_ids.contains(from) || !node_ids.contains(to) {
            return Err(TaskGroupError::InvalidGraph(format!(
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
        .filter_map(|(node_id, count)| if *count == 0 { Some(node_id.clone()) } else { None })
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
        return Err(TaskGroupError::InvalidGraph(
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
    if !statuses.is_empty() && statuses.iter().all(|status| *status == TaskStatus::Cancelled) {
        return TaskStatus::Cancelled;
    }
    TaskStatus::Todo
}

impl TaskGroup {
    fn parse_graph(graph_json: serde_json::Value) -> Result<TaskGroupGraph, TaskGroupError> {
        Ok(serde_json::from_value(graph_json)?)
    }

    async fn build_node_status_map<C: ConnectionTrait>(
        db: &C,
        nodes: &[TaskGroupNode],
    ) -> Result<HashMap<Uuid, TaskStatus>, TaskGroupError> {
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
        mut graph: TaskGroupGraph,
        status_map: &HashMap<Uuid, TaskStatus>,
    ) -> (TaskGroupGraph, Vec<TaskStatus>) {
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
        model: task_group::Model,
    ) -> Result<Self, TaskGroupError> {
        let project_uuid = ids::project_uuid_by_id(db, model.project_id)
            .await?
            .ok_or(TaskGroupError::ProjectNotFound)?;
        let graph = Self::parse_graph(model.graph_json)?;
        let status_map = Self::build_node_status_map(db, &graph.nodes).await?;
        let (graph_with_status, node_statuses) = Self::apply_node_statuses(graph, &status_map);
        let suggested_status = aggregate_status(&node_statuses);

        Ok(Self {
            id: model.uuid,
            project_id: project_uuid,
            title: model.title,
            description: model.description,
            status: model.status,
            baseline_ref: model.baseline_ref,
            schema_version: model.schema_version,
            graph: graph_with_status,
            suggested_status,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        })
    }

    pub async fn find_by_id<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<Option<Self>, TaskGroupError> {
        let record = task_group::Entity::find()
            .filter(task_group::Column::Uuid.eq(id))
            .one(db)
            .await?;

        match record {
            Some(model) => Ok(Some(Self::from_model(db, model).await?)),
            None => Ok(None),
        }
    }

    pub async fn find_by_project_id<C: ConnectionTrait>(
        db: &C,
        project_id: Uuid,
    ) -> Result<Vec<Self>, TaskGroupError> {
        let project_row_id = ids::project_id_by_uuid(db, project_id)
            .await?
            .ok_or(TaskGroupError::ProjectNotFound)?;

        let models = task_group::Entity::find()
            .filter(task_group::Column::ProjectId.eq(project_row_id))
            .order_by_desc(task_group::Column::CreatedAt)
            .all(db)
            .await?;

        let mut groups = Vec::with_capacity(models.len());
        for model in models {
            groups.push(Self::from_model(db, model).await?);
        }
        Ok(groups)
    }

    pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, TaskGroupError> {
        let models = task_group::Entity::find()
            .order_by_desc(task_group::Column::CreatedAt)
            .all(db)
            .await?;

        let mut groups = Vec::with_capacity(models.len());
        for model in models {
            groups.push(Self::from_model(db, model).await?);
        }
        Ok(groups)
    }

    pub async fn create<C: ConnectionTrait>(
        db: &C,
        data: &CreateTaskGroup,
        task_group_id: Uuid,
    ) -> Result<Self, TaskGroupError> {
        validate_schema_version(data.schema_version)?;
        validate_graph(&data.graph)?;

        let project_row_id = ids::project_id_by_uuid(db, data.project_id)
            .await?
            .ok_or(TaskGroupError::ProjectNotFound)?;

        let graph_json = serde_json::to_value(data.graph.without_statuses())?;
        let now = Utc::now();
        let active = task_group::ActiveModel {
            uuid: Set(task_group_id),
            project_id: Set(project_row_id),
            title: Set(data.title.clone()),
            description: Set(data.description.clone()),
            status: Set(data.status.clone().unwrap_or_default()),
            baseline_ref: Set(data.baseline_ref.clone()),
            schema_version: Set(data.schema_version),
            graph_json: Set(graph_json),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };

        let model = active.insert(db).await?;

        Self::sync_task_links(db, model.id, project_row_id, &data.graph).await?;
        Self::sync_entry_task_statuses_by_row_id(db, model.id).await?;

        Self::from_model(db, model).await
    }

    pub async fn update<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
        data: &UpdateTaskGroup,
    ) -> Result<Self, TaskGroupError> {
        let record = task_group::Entity::find()
            .filter(task_group::Column::Uuid.eq(id))
            .one(db)
            .await?
            .ok_or(TaskGroupError::TaskGroupNotFound)?;

        if let Some(schema_version) = data.schema_version {
            validate_schema_version(schema_version)?;
        }

        let mut title = record.title.clone();
        let mut description = record.description.clone();
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
        if let Some(value) = data.status.clone() {
            status = value;
        }
        if let Some(value) = &data.baseline_ref {
            baseline_ref = value.clone();
        }
        if let Some(value) = data.schema_version {
            schema_version = value;
        }
        if let Some(value) = &data.graph {
            validate_graph(value)?;
            graph = value.clone();
        }

        let mut active: task_group::ActiveModel = record.into();
        active.title = Set(title);
        active.description = Set(description);
        active.status = Set(status);
        active.baseline_ref = Set(baseline_ref);
        active.schema_version = Set(schema_version);
        active.graph_json = Set(serde_json::to_value(graph.without_statuses())?);
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;

        if let Some(graph) = &data.graph {
            Self::sync_task_links(db, updated.id, updated.project_id, graph).await?;
            Self::sync_entry_task_statuses_by_row_id(db, updated.id).await?;
        }

        Self::from_model(db, updated).await
    }

    pub async fn delete<C: ConnectionTrait>(
        db: &C,
        id: Uuid,
    ) -> Result<u64, TaskGroupError> {
        let record = task_group::Entity::find()
            .filter(task_group::Column::Uuid.eq(id))
            .one(db)
            .await?;

        let Some(record) = record else {
            return Ok(0);
        };

        let group_row_id = record.id;
        let tasks = task::Entity::find()
            .filter(task::Column::TaskGroupId.eq(group_row_id))
            .all(db)
            .await?;

        for task_model in tasks {
            Self::clear_task_link(db, &task_model).await?;
        }

        let result = task_group::Entity::delete_many()
            .filter(task_group::Column::Uuid.eq(id))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    pub async fn sync_entry_task_statuses_by_row_id<C: ConnectionTrait>(
        db: &C,
        task_group_row_id: i64,
    ) -> Result<TaskStatus, TaskGroupError> {
        let record = task_group::Entity::find_by_id(task_group_row_id)
            .one(db)
            .await?
            .ok_or(TaskGroupError::TaskGroupNotFound)?;

        let graph = Self::parse_graph(record.graph_json)?;
        let status_map = Self::build_node_status_map(db, &graph.nodes).await?;
        let (_, node_statuses) = Self::apply_node_statuses(graph, &status_map);
        let suggested_status = aggregate_status(&node_statuses);

        let entry_tasks = task::Entity::find()
            .filter(task::Column::TaskGroupId.eq(task_group_row_id))
            .filter(task::Column::TaskKind.eq(TaskKind::Group))
            .all(db)
            .await?;

        for task_model in entry_tasks {
            if task_model.status != suggested_status {
                let mut active: task::ActiveModel = task_model.clone().into();
                active.status = Set(suggested_status.clone());
                active.updated_at = Set(Utc::now().into());
                let updated = active.update(db).await?;
                Self::enqueue_task_updated(db, &updated).await?;
            }
        }

        Ok(suggested_status)
    }

    async fn sync_task_links<C: ConnectionTrait>(
        db: &C,
        task_group_row_id: i64,
        project_row_id: i64,
        graph: &TaskGroupGraph,
    ) -> Result<(), TaskGroupError> {
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
                    return Err(TaskGroupError::TaskNotFound(task_id.to_string()));
                }
            }

            for node in &graph.nodes {
                let task_model = task_map
                    .get(&node.task_id)
                    .ok_or_else(|| TaskGroupError::TaskNotFound(node.task_id.to_string()))?;

                if task_model.project_id != project_row_id {
                    return Err(TaskGroupError::TaskProjectMismatch(node.task_id.to_string()));
                }
                if task_model.task_group_id.is_some()
                    && task_model.task_group_id != Some(task_group_row_id)
                {
                    return Err(TaskGroupError::TaskGroupMismatch(node.task_id.to_string()));
                }
                if task_model.task_kind == TaskKind::Group {
                    return Err(TaskGroupError::TaskKindMismatch(node.task_id.to_string()));
                }
            }

            let mut node_id_map: HashMap<Uuid, String> = HashMap::new();
            for node in &graph.nodes {
                node_id_map.insert(node.task_id, node.id.trim().to_string());
            }

            for (task_id, node_id) in &node_id_map {
                let task_model = task_map
                    .get(task_id)
                    .ok_or_else(|| TaskGroupError::TaskNotFound(task_id.to_string()))?;
                let mut active: task::ActiveModel = task_model.clone().into();
                active.task_group_id = Set(Some(task_group_row_id));
                active.task_group_node_id = Set(Some(node_id.clone()));
                active.updated_at = Set(Utc::now().into());
                let updated = active.update(db).await?;
                Self::enqueue_task_updated(db, &updated).await?;
            }
        }

        let current_group_tasks = task::Entity::find()
            .filter(task::Column::TaskGroupId.eq(task_group_row_id))
            .filter(task::Column::TaskKind.ne(TaskKind::Group))
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
    ) -> Result<(), TaskGroupError> {
        let mut active: task::ActiveModel = task_model.clone().into();
        active.task_group_id = Set(None);
        active.task_group_node_id = Set(None);
        active.updated_at = Set(Utc::now().into());
        let updated = active.update(db).await?;
        Self::enqueue_task_updated(db, &updated).await?;
        Ok(())
    }

    async fn enqueue_task_updated<C: ConnectionTrait>(
        db: &C,
        task_model: &task::Model,
    ) -> Result<(), TaskGroupError> {
        let project_uuid = ids::project_uuid_by_id(db, task_model.project_id)
            .await?
            .ok_or(TaskGroupError::ProjectNotFound)?;
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
    use super::*;

    fn base_graph() -> TaskGroupGraph {
        TaskGroupGraph {
            nodes: vec![
                TaskGroupNode {
                    id: "node-a".to_string(),
                    task_id: Uuid::new_v4(),
                    kind: TaskGroupNodeKind::Task,
                    phase: 0,
                    agent_role: None,
                    cost_estimate: None,
                    artifacts: Vec::new(),
                    instructions: None,
                    requires_approval: None,
                    layout: TaskGroupNodeLayout { x: 0.0, y: 0.0 },
                    status: None,
                },
                TaskGroupNode {
                    id: "node-b".to_string(),
                    task_id: Uuid::new_v4(),
                    kind: TaskGroupNodeKind::Task,
                    phase: 0,
                    agent_role: None,
                    cost_estimate: None,
                    artifacts: Vec::new(),
                    instructions: None,
                    requires_approval: None,
                    layout: TaskGroupNodeLayout { x: 1.0, y: 1.0 },
                    status: None,
                },
            ],
            edges: vec![TaskGroupEdge {
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
        assert!(matches!(err, TaskGroupError::InvalidGraph(_)));
    }

    #[test]
    fn validate_graph_rejects_self_edges() {
        let mut graph = base_graph();
        graph.edges[0].from = "node-a".to_string();
        graph.edges[0].to = "node-a".to_string();
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, TaskGroupError::InvalidGraph(_)));
    }

    #[test]
    fn validate_graph_rejects_missing_nodes() {
        let mut graph = base_graph();
        graph.edges[0].to = "missing".to_string();
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, TaskGroupError::InvalidGraph(_)));
    }

    #[test]
    fn validate_graph_rejects_cycles() {
        let mut graph = base_graph();
        graph.edges.push(TaskGroupEdge {
            id: "edge-b-a".to_string(),
            from: "node-b".to_string(),
            to: "node-a".to_string(),
            data_flow: None,
        });
        let err = validate_graph(&graph).unwrap_err();
        assert!(matches!(err, TaskGroupError::InvalidGraph(_)));
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
}
