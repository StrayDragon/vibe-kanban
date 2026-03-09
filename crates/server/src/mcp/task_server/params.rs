use super::*;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTaskRequest {
    #[schemars(
        description = "The ID of the project to create the task in (UUID string). This is required!"
    )]
    pub project_id: Uuid,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Optional description of the task")]
    pub description: Option<String>,
    #[schemars(
        description = "Optional origin task id (UUID string) when this task is created as a follow-up to another task"
    )]
    pub origin_task_id: Option<Uuid>,
    #[schemars(
        description = "Optional task source kind: 'human_ui', 'mcp', 'scheduler', or 'agent_followup'"
    )]
    pub created_by_kind: Option<String>,
    #[schemars(
        description = "Optional idempotency key for safe retries. When provided, repeated calls with the same key and same payload return the same result."
    )]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateTaskResponse {
    #[schemars(description = "The unique identifier of the created task (UUID string)")]
    pub task_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateTaskRequest {
    #[schemars(description = "The ID of the task to update (UUID string)")]
    pub task_id: Uuid,
    #[schemars(description = "New title for the task")]
    pub title: Option<String>,
    #[schemars(description = "New description for the task")]
    pub description: Option<String>,
    #[schemars(description = "New status: 'todo', 'inprogress', 'inreview', 'done', 'cancelled'")]
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateTaskResponse {
    #[schemars(description = "The updated task id (UUID string)")]
    pub task_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteTaskRequest {
    #[schemars(description = "The ID of the task to delete (UUID string)")]
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteTaskResponse {
    #[schemars(description = "The deleted task id (UUID string)")]
    pub deleted_task_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectSummary {
    #[schemars(description = "The unique identifier of the project (UUID string)")]
    pub id: String,
    #[schemars(description = "The name of the project")]
    pub name: String,
    #[schemars(description = "When the project was created")]
    pub created_at: String,
    #[schemars(description = "When the project was last updated")]
    pub updated_at: String,
}

impl ProjectSummary {
    pub(super) fn from_project(project: Project) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListProjectsResponse {
    #[schemars(description = "Project summaries")]
    pub projects: Vec<ProjectSummary>,
    #[schemars(description = "Number of projects returned")]
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpRepoSummary {
    #[schemars(description = "The unique identifier of the repository (UUID string)")]
    pub id: String,
    #[schemars(description = "The name of the repository")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListReposRequest {
    #[schemars(description = "The ID of the project to list repositories from (UUID string)")]
    pub project_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListReposResponse {
    #[schemars(description = "Repository summaries for the project")]
    pub repos: Vec<McpRepoSummary>,
    #[schemars(description = "Number of repositories returned")]
    pub count: usize,
    #[schemars(description = "The project identifier used for the query (UUID string)")]
    pub project_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpExecutorSummary {
    #[schemars(description = "Stable executor identifier (use as start_attempt.executor)")]
    pub executor: String,
    #[schemars(
        description = "Available executor variants (excluding the implicit default). Provide as start_attempt.variant."
    )]
    pub variants: Vec<String>,
    #[schemars(description = "Whether this executor supports MCP configuration")]
    pub supports_mcp: bool,
    #[schemars(description = "Default variant to use, or null to omit variant")]
    pub default_variant: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListExecutorsResponse {
    #[schemars(description = "Available executors")]
    pub executors: Vec<McpExecutorSummary>,
    #[schemars(description = "Number of executors returned")]
    pub count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTasksRequest {
    #[schemars(description = "The ID of the project to list tasks from (UUID string)")]
    pub project_id: Uuid,
    #[schemars(
        description = "Optional status filter: 'todo', 'inprogress', 'inreview', 'done', 'cancelled'"
    )]
    pub status: Option<String>,
    #[schemars(description = "Maximum number of tasks to return (default: 50)")]
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskSummary {
    #[schemars(description = "The unique identifier of the task (UUID string)")]
    pub id: String,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Current status of the task")]
    pub status: String,
    #[schemars(description = "Task kind: default or group")]
    pub task_kind: String,
    #[schemars(
        description = "Owning milestone/task group id if the task is part of a milestone node (UUID string)"
    )]
    pub task_group_id: Option<String>,
    #[schemars(description = "Owning milestone node id if the task is part of a milestone node")]
    pub task_group_node_id: Option<String>,
    #[schemars(description = "Task source kind: human_ui, mcp, scheduler, or agent_followup")]
    pub created_by_kind: String,
    #[schemars(
        description = "Origin task id when this task was created as follow-up work (UUID string)"
    )]
    pub origin_task_id: Option<String>,
    #[schemars(description = "When the task was created")]
    pub created_at: String,
    #[schemars(description = "When the task was last updated")]
    pub updated_at: String,
    #[schemars(description = "Latest attempt id for this task (UUID string)")]
    pub latest_attempt_id: Option<String>,
    #[schemars(description = "Workspace branch for the latest attempt")]
    pub latest_workspace_branch: Option<String>,
    #[schemars(description = "Latest session id for the latest attempt (UUID string)")]
    pub latest_session_id: Option<String>,
    #[schemars(description = "Executor for the latest session of the latest attempt")]
    pub latest_session_executor: Option<String>,
    #[schemars(description = "Whether the task has an in-progress execution attempt")]
    pub has_in_progress_attempt: bool,
    #[schemars(description = "Whether the last execution attempt failed")]
    pub last_attempt_failed: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TaskAttemptSummary {
    pub(super) latest_attempt_id: Option<String>,
    pub(super) latest_workspace_branch: Option<String>,
    pub(super) latest_session_id: Option<String>,
    pub(super) latest_session_executor: Option<String>,
}

impl TaskSummary {
    pub(super) fn from_task_with_status(
        task: TaskWithAttemptStatus,
        summary: TaskAttemptSummary,
    ) -> Self {
        let TaskWithAttemptStatus {
            task,
            has_in_progress_attempt,
            last_attempt_failed,
            ..
        } = task;
        Self {
            id: task.id.to_string(),
            title: task.title,
            status: task.status.to_string(),
            task_kind: task.task_kind.to_string(),
            task_group_id: task.task_group_id.map(|id| id.to_string()),
            task_group_node_id: task.task_group_node_id.clone(),
            created_by_kind: task.created_by_kind.to_string(),
            origin_task_id: task.origin_task_id.map(|id| id.to_string()),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            latest_attempt_id: summary.latest_attempt_id,
            latest_workspace_branch: summary.latest_workspace_branch,
            latest_session_id: summary.latest_session_id,
            latest_session_executor: summary.latest_session_executor,
            has_in_progress_attempt,
            last_attempt_failed,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListTasksResponse {
    #[schemars(description = "Tasks")]
    pub tasks: Vec<TaskSummary>,
    #[schemars(description = "Number of tasks returned")]
    pub count: usize,
    #[schemars(description = "The project identifier used for the query (UUID string)")]
    pub project_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListArchivedKanbansRequest {
    #[schemars(description = "The project identifier to list archives for (UUID string).")]
    pub project_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpArchivedKanban {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub tasks_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

impl McpArchivedKanban {
    pub(super) fn from_model(model: ArchivedKanbanWithTaskCount) -> Self {
        Self {
            id: model.archived_kanban.id.to_string(),
            project_id: model.archived_kanban.project_id.to_string(),
            title: model.archived_kanban.title,
            tasks_count: model.tasks_count,
            created_at: model.archived_kanban.created_at.to_rfc3339(),
            updated_at: model.archived_kanban.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListArchivedKanbansResponse {
    #[schemars(description = "Project id (UUID string)")]
    pub project_id: String,
    #[schemars(description = "Archived kanban batches (newest first)")]
    pub archived_kanbans: Vec<McpArchivedKanban>,
    #[schemars(description = "Number of archived kanbans returned")]
    pub count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArchiveProjectKanbanRequest {
    #[schemars(description = "The ID of the project to archive tasks from (UUID string).")]
    pub project_id: Uuid,
    #[schemars(description = "Task statuses to archive (e.g. done, cancelled).")]
    pub statuses: Vec<String>,
    #[schemars(description = "Optional archive title.")]
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArchiveProjectKanbanResponse {
    #[schemars(description = "The created archived kanban batch.")]
    pub archived_kanban: McpArchivedKanban,
    #[schemars(description = "Number of tasks moved into the archive.")]
    pub moved_task_count: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestoreArchivedKanbanRequest {
    #[schemars(description = "The archived kanban id to restore from (UUID string).")]
    pub archive_id: Uuid,
    #[schemars(description = "If true, restore all tasks in this archive.")]
    pub restore_all: Option<bool>,
    #[schemars(description = "Optional status filter when restore_all=false.")]
    pub statuses: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RestoreArchivedKanbanResponse {
    #[schemars(description = "Archived kanban id (UUID string)")]
    pub archive_id: String,
    #[schemars(description = "Number of tasks restored to active set.")]
    pub restored_task_count: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetTaskRequest {
    #[schemars(description = "The ID of the task to retrieve (UUID string)")]
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpTask {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub task_kind: String,
    pub task_group_id: Option<String>,
    pub task_group_node_id: Option<String>,
    pub created_by_kind: String,
    pub origin_task_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl McpTask {
    pub(super) fn from_task_with_status(task: TaskWithAttemptStatus) -> Self {
        let TaskWithAttemptStatus { task, .. } = task;
        Self {
            id: task.id.to_string(),
            project_id: task.project_id.to_string(),
            title: task.title,
            description: task.description,
            status: task.status.to_string(),
            task_kind: task.task_kind.to_string(),
            task_group_id: task.task_group_id.map(|id| id.to_string()),
            task_group_node_id: task.task_group_node_id.clone(),
            created_by_kind: task.created_by_kind.to_string(),
            origin_task_id: task.origin_task_id.map(|id| id.to_string()),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetTaskResponse {
    #[schemars(description = "Task details")]
    pub task: McpTask,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListTaskAttemptsRequest {
    #[schemars(description = "The ID of the task to list attempts for (UUID string)")]
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AttemptSummary {
    #[schemars(description = "Workspace/attempt id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Workspace branch name")]
    pub workspace_branch: String,
    #[schemars(description = "When the attempt was created (RFC3339)")]
    pub created_at: String,
    #[schemars(description = "When the attempt was last updated (RFC3339)")]
    pub updated_at: String,
    #[schemars(description = "Latest session id for the attempt (UUID string)")]
    pub latest_session_id: Option<String>,
    #[schemars(description = "Executor for the latest session")]
    pub latest_session_executor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListTaskAttemptsResponse {
    #[schemars(description = "Task id (UUID string)")]
    pub task_id: String,
    #[schemars(description = "Attempts (newest first)")]
    pub attempts: Vec<AttemptSummary>,
    #[schemars(description = "Number of attempts returned")]
    pub count: usize,
    #[schemars(description = "Latest attempt id if present (UUID string)")]
    pub latest_attempt_id: Option<String>,
    #[schemars(description = "Latest session id if present (UUID string)")]
    pub latest_session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRepoInput {
    #[schemars(description = "Repo id (UUID string)")]
    pub repo_id: Uuid,
    #[schemars(description = "Target branch name for this repo")]
    pub target_branch: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartAttemptRequest {
    #[schemars(description = "The task id to start an attempt for (UUID string)")]
    pub task_id: Uuid,
    #[schemars(description = "Executor name (e.g., CLAUDE_CODE)")]
    pub executor: String,
    #[schemars(description = "Optional executor variant")]
    pub variant: Option<String>,
    #[schemars(description = "Workspace repos (repo_id + target_branch)")]
    pub repos: Vec<WorkspaceRepoInput>,
    #[schemars(
        description = "Optional idempotency key for safe retries. When provided, repeated calls with the same key and same payload return the same result."
    )]
    pub request_id: Option<String>,
    #[schemars(
        description = "Optional prompt override. When provided, this prompt is used as the initial agent prompt instead of the task title/description."
    )]
    pub prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StartAttemptResponse {
    #[schemars(description = "Attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Session id created for the attempt (UUID string)")]
    pub session_id: String,
    #[schemars(description = "Initial execution process id (UUID string)")]
    pub execution_process_id: String,
    #[schemars(description = "Attempt control token (lease bearer token)")]
    pub control_token: String,
    #[schemars(description = "When the control lease expires (RFC3339)")]
    pub control_expires_at: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimAttemptControlRequest {
    #[schemars(description = "Attempt/workspace id (UUID string)")]
    pub attempt_id: Uuid,
    #[schemars(description = "Optional lease TTL in seconds (default: 3600; max: 86400)")]
    pub ttl_secs: Option<i64>,
    #[schemars(
        description = "If true, force-claim even when another client holds an unexpired lease"
    )]
    pub force: Option<bool>,
    #[schemars(
        description = "Optional client id for audit/coordination (default: derived from MCP peer info)"
    )]
    pub claimed_by_client_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ClaimAttemptControlResponse {
    pub attempt_id: String,
    pub control_token: String,
    pub claimed_by_client_id: String,
    pub expires_at: String,
    pub token_rotated: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetAttemptControlRequest {
    pub attempt_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptControlResponse {
    pub attempt_id: String,
    pub has_lease: bool,
    pub claimed_by_client_id: Option<String>,
    pub expires_at: Option<String>,
    pub expired: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseAttemptControlRequest {
    pub attempt_id: Uuid,
    pub control_token: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReleaseAttemptControlResponse {
    pub attempt_id: String,
    pub released: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SendFollowUpRequest {
    #[schemars(
        description = "Attempt/workspace id (UUID string). Provide exactly one of attempt_id or session_id."
    )]
    pub attempt_id: Option<Uuid>,
    #[schemars(
        description = "Session id (UUID string). Provide exactly one of attempt_id or session_id."
    )]
    pub session_id: Option<Uuid>,
    #[schemars(
        description = "Attempt control token (lease bearer token). Obtain via start_attempt or claim_attempt_control."
    )]
    pub control_token: Option<Uuid>,
    #[schemars(description = "Follow-up prompt to send")]
    pub prompt: String,
    #[schemars(description = "Optional executor variant override")]
    pub variant: Option<String>,
    #[schemars(
        description = "Optional idempotency key for safe retries. When provided, repeated calls with the same key and same payload return the same result."
    )]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SendFollowUpResponse {
    #[schemars(description = "Session id used (UUID string)")]
    pub session_id: String,
    #[schemars(description = "Execution process id started for this follow-up (UUID string)")]
    pub execution_process_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StopAttemptRequest {
    #[schemars(description = "The attempt/workspace id (UUID string). This is required!")]
    pub attempt_id: Uuid,
    #[schemars(
        description = "Attempt control token (lease bearer token). Obtain via start_attempt or claim_attempt_control."
    )]
    pub control_token: Option<Uuid>,
    #[schemars(description = "If true, perform a hard stop (default: false).")]
    pub force: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StopAttemptResponse {
    #[schemars(description = "Attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Whether force was applied")]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum McpAttemptState {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpIndexedLogEntry {
    #[schemars(description = "Monotonic log entry index")]
    pub entry_index: i64,
    #[schemars(
        description = "Log entry payload (normalized PatchType JSON). Treat as opaque JSON and render/inspect by `type`."
    )]
    pub entry: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpLogHistoryPage {
    #[schemars(description = "Entries in chronological order (oldest→newest)")]
    pub entries: Vec<McpIndexedLogEntry>,
    #[schemars(
        description = "Cursor to request the next older page. Pass as `cursor` to a follow-up call."
    )]
    pub next_cursor: Option<i64>,
    #[schemars(description = "Whether older history exists beyond this page")]
    pub has_more: bool,
    #[schemars(description = "Whether history was truncated due to in-memory eviction")]
    pub history_truncated: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TailSessionMessagesRequest {
    #[schemars(
        description = "Attempt/workspace id (UUID string). Provide exactly one of attempt_id or session_id."
    )]
    pub attempt_id: Option<Uuid>,
    #[schemars(
        description = "Session id (UUID string). Provide exactly one of attempt_id or session_id."
    )]
    pub session_id: Option<Uuid>,
    #[schemars(description = "Maximum number of entries to return (default: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Cursor to request older history")]
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpSessionMessageTurn {
    pub entry_index: i64,
    pub turn_id: String,
    pub prompt: Option<String>,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpSessionMessagesPage {
    pub entries: Vec<McpSessionMessageTurn>,
    pub next_cursor: Option<i64>,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TailSessionMessagesResponse {
    pub session_id: String,
    pub page: McpSessionMessagesPage,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TailAttemptFeedRequest {
    #[schemars(description = "Attempt/workspace id (UUID string)")]
    pub attempt_id: Uuid,
    #[schemars(description = "Maximum number of log entries to return (default: 50)")]
    pub limit: Option<usize>,
    #[schemars(description = "Cursor to request older history")]
    pub cursor: Option<i64>,
    #[schemars(description = "Return only log entries newer than this index")]
    pub after_log_index: Option<i64>,
    #[schemars(
        description = "Optional long-poll wait in milliseconds (only valid when after_log_index is set; max 30000)"
    )]
    pub wait_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpApprovalSummary {
    pub approval_id: String,
    pub attempt_id: String,
    pub execution_process_id: String,
    pub tool_name: String,
    pub tool_call_id: String,
    pub tool_input: Value,
    pub status: String,
    pub created_at: String,
    pub timeout_at: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TailAttemptFeedResponse {
    pub attempt_id: String,
    pub task_id: String,
    pub workspace_branch: String,
    pub state: McpAttemptState,
    pub latest_session_id: Option<String>,
    pub latest_execution_process_id: Option<String>,
    pub failure_summary: Option<String>,
    pub page: McpLogHistoryPage,
    pub next_after_log_index: Option<i64>,
    pub pending_approvals: Vec<McpApprovalSummary>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpAttemptChangesBlockedReason {
    SummaryFailed,
    ThresholdExceeded,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpAttemptChangesSummary {
    pub file_count: usize,
    pub added: usize,
    pub deleted: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptChangesResponse {
    pub attempt_id: String,
    pub summary: McpAttemptChangesSummary,
    pub blocked: bool,
    pub blocked_reason: Option<McpAttemptChangesBlockedReason>,
    pub code: Option<String>,
    pub retryable: Option<bool>,
    pub hint: Option<String>,
    pub files: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetAttemptChangesRequest {
    pub attempt_id: Uuid,
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpAttemptArtifactBlockedReason {
    PathOutsideWorkspace,
    SizeExceeded,
    TooManyPaths,
    SummaryFailed,
    ThresholdExceeded,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetAttemptFileRequest {
    pub attempt_id: Uuid,
    pub path: String,
    pub start: Option<u64>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptFileResponse {
    pub attempt_id: String,
    pub blocked: bool,
    pub blocked_reason: Option<McpAttemptArtifactBlockedReason>,
    pub code: Option<String>,
    pub retryable: Option<bool>,
    pub hint: Option<String>,
    pub truncated: bool,
    pub start: u64,
    pub bytes: usize,
    pub total_bytes: Option<u64>,
    pub path: String,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetAttemptPatchRequest {
    pub attempt_id: Uuid,
    pub paths: Vec<String>,
    pub force: Option<bool>,
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptPatchResponse {
    pub attempt_id: String,
    pub blocked: bool,
    pub blocked_reason: Option<McpAttemptArtifactBlockedReason>,
    pub code: Option<String>,
    pub retryable: Option<bool>,
    pub hint: Option<String>,
    pub truncated: bool,
    pub bytes: usize,
    pub paths: Vec<String>,
    pub patch: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListApprovalsRequest {
    pub attempt_id: Uuid,
    #[schemars(description = "Optional status filter: pending|approved|denied|timed_out")]
    pub status: Option<String>,
    #[schemars(description = "Maximum number of approvals to return (default: 50; max: 200)")]
    pub limit: Option<u64>,
    #[schemars(description = "Cursor for older paging (approval db id)")]
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListApprovalsResponse {
    pub attempt_id: String,
    pub approvals: Vec<McpApprovalSummary>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetApprovalRequest {
    pub approval_id: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetApprovalResponse {
    pub approval: McpApprovalSummary,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RespondApprovalRequest {
    pub approval_id: String,
    pub execution_process_id: Uuid,
    #[schemars(description = "approved|denied|timed_out")]
    pub status: String,
    pub denial_reason: Option<String>,
    pub responded_by_client_id: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RespondApprovalResponse {
    pub approval_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TailProjectActivityRequest {
    pub project_id: Uuid,
    pub limit: Option<u64>,
    pub cursor: Option<i64>,
    pub after_event_id: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TailTaskActivityRequest {
    pub task_id: Uuid,
    pub limit: Option<u64>,
    pub cursor: Option<i64>,
    pub after_event_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ActivityEvent {
    pub event_id: i64,
    pub event_uuid: String,
    pub event_type: String,
    pub entity_type: String,
    pub entity_uuid: String,
    pub created_at: String,
    pub published_at: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TailActivityResponse {
    pub events: Vec<ActivityEvent>,
    pub next_cursor: Option<i64>,
    pub has_more: bool,
    pub next_after_event_id: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CliDependencyPreflightRequest {
    #[schemars(description = "Optional list of binary names to check (default: common deps)")]
    pub binaries: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CliDependencyCheck {
    pub name: String,
    pub ok: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CliDependencyPreflightResponse {
    pub all_ok: bool,
    pub checks: Vec<CliDependencyCheck>,
}
