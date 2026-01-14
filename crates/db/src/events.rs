use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const EVENT_TASK_CREATED: &str = "task.created";
pub const EVENT_TASK_UPDATED: &str = "task.updated";
pub const EVENT_TASK_DELETED: &str = "task.deleted";

pub const EVENT_PROJECT_CREATED: &str = "project.created";
pub const EVENT_PROJECT_UPDATED: &str = "project.updated";
pub const EVENT_PROJECT_DELETED: &str = "project.deleted";

pub const EVENT_WORKSPACE_CREATED: &str = "workspace.created";
pub const EVENT_WORKSPACE_UPDATED: &str = "workspace.updated";
pub const EVENT_WORKSPACE_DELETED: &str = "workspace.deleted";

pub const EVENT_EXECUTION_PROCESS_CREATED: &str = "execution_process.created";
pub const EVENT_EXECUTION_PROCESS_UPDATED: &str = "execution_process.updated";
pub const EVENT_EXECUTION_PROCESS_DELETED: &str = "execution_process.deleted";

pub const EVENT_SCRATCH_CREATED: &str = "scratch.created";
pub const EVENT_SCRATCH_UPDATED: &str = "scratch.updated";
pub const EVENT_SCRATCH_DELETED: &str = "scratch.deleted";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEventPayload {
    pub task_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEventPayload {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEventPayload {
    pub workspace_id: Uuid,
    pub task_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProcessEventPayload {
    pub process_id: Uuid,
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchEventPayload {
    pub scratch_id: Uuid,
    pub scratch_type: String,
}
