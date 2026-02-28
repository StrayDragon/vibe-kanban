use std::{borrow::Cow, cmp::Ordering, str::FromStr};

use db::models::{
    execution_process::ExecutionProcess,
    project::Project,
    repo::Repo,
    session::Session,
    tag::Tag,
    task::{CreateTask, Task, TaskStatus, TaskWithAttemptStatus, UpdateTask},
    workspace::{Workspace, WorkspaceContext},
};
use executors::{
    executors::{BaseCodingAgent, CodingAgent},
    profile::ExecutorProfileId,
};
use regex::Regex;
use reqwest::StatusCode;
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::tool::ToolRouter,
    model::{
        CallToolResult, Content, Icon, Implementation, ProtocolVersion, ServerCapabilities,
        ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{self, Value};
use services::services::queued_message::{QueueStatus, QueuedMessage};
use strum::VariantNames;
use uuid::Uuid;

use crate::{
    mcp::params::Parameters,
    routes::{
        containers::ContainerQuery,
        task_attempts::{
            AttemptChangesBlockedReason as ApiAttemptChangesBlockedReason,
            AttemptArtifactBlockedReason as ApiAttemptArtifactBlockedReason,
            AttemptState as ApiAttemptState, CreateTaskAttemptBody,
            AttemptFileResponse as ApiAttemptFileResponse,
            AttemptPatchResponse as ApiAttemptPatchResponse,
            TaskAttemptChangesResponse as ApiTaskAttemptChangesResponse,
            TaskAttemptStatusResponse as ApiTaskAttemptStatusResponse, WorkspaceRepoInput,
        },
    },
};

const MCP_CODE_AMBIGUOUS_TARGET: &str = "ambiguous_target";
const MCP_CODE_NO_SESSION_YET: &str = "no_session_yet";
const MCP_CODE_BLOCKED_GUARDRAILS: &str = "blocked_guardrails";
const MCP_CODE_MIXED_PAGINATION: &str = "mixed_pagination";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
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
        description = "Optional idempotency key for safe retries. When provided, repeated calls with the same key and same payload return the same result."
    )]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct CreateTaskResponse {
    #[schemars(description = "The unique identifier of the created task (UUID string)")]
    pub task_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
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
    fn from_project(project: Project) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name,
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpRepoSummary {
    #[schemars(description = "The unique identifier of the repository (UUID string)")]
    pub id: String,
    #[schemars(description = "The name of the repository")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListReposRequest {
    #[schemars(description = "The ID of the project to list repositories from (UUID string)")]
    pub project_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListReposResponse {
    #[schemars(description = "Repository summaries for the project")]
    pub repos: Vec<McpRepoSummary>,
    #[schemars(description = "Number of repositories returned")]
    pub count: usize,
    #[schemars(description = "The project identifier used for the query (UUID string)")]
    pub project_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListProjectsResponse {
    #[schemars(description = "Project summaries")]
    pub projects: Vec<ProjectSummary>,
    #[schemars(description = "Number of projects returned")]
    pub count: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpExecutorSummary {
    #[schemars(description = "Stable executor identifier (use as start_task_attempt.executor)")]
    pub executor: String,
    #[schemars(
        description = "Available executor variants (excluding the implicit default). Provide as start_task_attempt.variant."
    )]
    pub variants: Vec<String>,
    #[schemars(description = "Whether this executor supports MCP configuration")]
    pub supports_mcp: bool,
    #[schemars(description = "Default variant to use, or null to omit variant")]
    pub default_variant: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListExecutorsResponse {
    #[schemars(description = "Available executors")]
    pub executors: Vec<McpExecutorSummary>,
    #[schemars(description = "Number of executors returned")]
    pub count: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TaskSummary {
    #[schemars(description = "The unique identifier of the task (UUID string)")]
    pub id: String,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Current status of the task")]
    pub status: String,
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
struct TaskAttemptSummary {
    latest_attempt_id: Option<String>,
    latest_workspace_branch: Option<String>,
    latest_session_id: Option<String>,
    latest_session_executor: Option<String>,
}

impl TaskSummary {
    fn from_task_with_status(
        task: TaskWithAttemptStatus,
        attempt_summary: TaskAttemptSummary,
    ) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title.to_string(),
            status: task.status.to_string(),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            latest_attempt_id: attempt_summary.latest_attempt_id,
            latest_workspace_branch: attempt_summary.latest_workspace_branch,
            latest_session_id: attempt_summary.latest_session_id,
            latest_session_executor: attempt_summary.latest_session_executor,
            has_in_progress_attempt: task.has_in_progress_attempt,
            last_attempt_failed: task.last_attempt_failed,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TaskDetails {
    #[schemars(description = "The unique identifier of the task (UUID string)")]
    pub id: String,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Optional description of the task")]
    pub description: Option<String>,
    #[schemars(description = "Current status of the task")]
    pub status: String,
    #[schemars(description = "When the task was created")]
    pub created_at: String,
    #[schemars(description = "When the task was last updated")]
    pub updated_at: String,
    #[schemars(description = "Whether the task has an in-progress execution attempt")]
    pub has_in_progress_attempt: Option<bool>,
    #[schemars(description = "Whether the last execution attempt failed")]
    pub last_attempt_failed: Option<bool>,
}

impl TaskDetails {
    fn from_task(task: Task) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title,
            description: task.description,
            status: task.status.to_string(),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            has_in_progress_attempt: None,
            last_attempt_failed: None,
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListTasksResponse {
    #[schemars(description = "Task summaries with latest attempt/session info")]
    pub tasks: Vec<TaskSummary>,
    #[schemars(description = "Number of tasks returned")]
    pub count: usize,
    #[schemars(description = "The project identifier used for the query (UUID string)")]
    pub project_id: String,
    #[schemars(description = "Filters applied to the task listing")]
    pub applied_filters: ListTasksFilters,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListTasksFilters {
    #[schemars(description = "Status filter applied to the list, if any")]
    pub status: Option<String>,
    #[schemars(description = "Maximum number of tasks returned")]
    pub limit: i32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListTaskAttemptsRequest {
    #[schemars(description = "The ID of the task to list attempts for (UUID string)")]
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TaskAttemptDetails {
    #[schemars(description = "Attempt identifier (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Workspace branch name for this attempt")]
    pub workspace_branch: String,
    #[schemars(description = "When the attempt workspace was created (RFC3339)")]
    pub created_at: String,
    #[schemars(description = "When the attempt workspace was last updated (RFC3339)")]
    pub updated_at: String,
    #[schemars(description = "Latest session id for this attempt (UUID string)")]
    pub latest_session_id: Option<String>,
    #[schemars(description = "Executor for the latest session in this attempt")]
    pub latest_session_executor: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListTaskAttemptsResponse {
    #[schemars(description = "The task identifier used for the query (UUID string)")]
    pub task_id: String,
    #[schemars(description = "Number of attempts returned")]
    pub count: usize,
    #[schemars(description = "Attempts ordered by workspace creation time (newest first)")]
    pub attempts: Vec<TaskAttemptDetails>,
    #[schemars(description = "Latest attempt id (UUID string)")]
    pub latest_attempt_id: Option<String>,
    #[schemars(description = "Latest session id for the latest attempt (UUID string)")]
    pub latest_session_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct UpdateTaskResponse {
    #[schemars(description = "Updated task details")]
    pub task: TaskDetails,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteTaskRequest {
    #[schemars(description = "The ID of the task to delete (UUID string)")]
    pub task_id: Uuid,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct McpWorkspaceRepoInput {
    #[schemars(description = "The repository ID (UUID string)")]
    pub repo_id: Uuid,
    #[schemars(description = "The target branch for this repository")]
    pub target_branch: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartTaskAttemptRequest {
    #[schemars(description = "The ID of the task to start (UUID string)")]
    pub task_id: Uuid,
    #[schemars(
        description = "The coding agent executor to run ('CLAUDE_CODE', 'CODEX', 'GEMINI', 'CURSOR_AGENT', 'OPENCODE')"
    )]
    pub executor: String,
    #[schemars(description = "Optional executor variant, if needed")]
    pub variant: Option<String>,
    #[schemars(description = "Target branch for each repository in the project")]
    pub repos: Vec<McpWorkspaceRepoInput>,
    #[schemars(
        description = "Optional idempotency key for safe retries. When provided, repeated calls with the same key and same payload return the same attempt."
    )]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StartTaskAttemptResponse {
    #[schemars(description = "The task identifier for the new attempt (UUID string)")]
    pub task_id: String,
    #[schemars(description = "The created attempt identifier (UUID string)")]
    pub attempt_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FollowUpAction {
    Send,
    Queue,
    Cancel,
}

#[derive(Debug, Deserialize)]
pub struct FollowUpRequest {
    pub session_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub prompt: Option<String>,
    pub action: FollowUpAction,
    pub variant: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum FollowUpActionSendSchema {
    Send,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum FollowUpActionQueueSchema {
    Queue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum FollowUpActionCancelSchema {
    Cancel,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FollowUpSendByAttemptSchema {
    pub action: FollowUpActionSendSchema,
    pub attempt_id: Uuid,
    pub prompt: String,
    pub variant: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FollowUpSendBySessionSchema {
    pub action: FollowUpActionSendSchema,
    pub session_id: Uuid,
    pub prompt: String,
    pub variant: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FollowUpQueueByAttemptSchema {
    pub action: FollowUpActionQueueSchema,
    pub attempt_id: Uuid,
    pub prompt: String,
    pub variant: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FollowUpQueueBySessionSchema {
    pub action: FollowUpActionQueueSchema,
    pub session_id: Uuid,
    pub prompt: String,
    pub variant: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FollowUpCancelByAttemptSchema {
    pub action: FollowUpActionCancelSchema,
    pub attempt_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct FollowUpCancelBySessionSchema {
    pub action: FollowUpActionCancelSchema,
    pub session_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum FollowUpRequestSchema {
    SendByAttempt(FollowUpSendByAttemptSchema),
    SendBySession(FollowUpSendBySessionSchema),
    QueueByAttempt(FollowUpQueueByAttemptSchema),
    QueueBySession(FollowUpQueueBySessionSchema),
    CancelByAttempt(FollowUpCancelByAttemptSchema),
    CancelBySession(FollowUpCancelBySessionSchema),
}

impl schemars::JsonSchema for FollowUpRequest {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("FollowUpRequest")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        FollowUpRequestSchema::json_schema(generator)
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct FollowUpResponse {
    #[schemars(description = "The session id used for the follow-up (UUID string)")]
    pub session_id: String,
    #[schemars(description = "The follow-up action that was performed")]
    pub action: FollowUpAction,
    #[schemars(description = "Execution process id if a follow-up run was started (UUID string)")]
    pub execution_process_id: Option<String>,
    #[schemars(description = "Queue status when queue/cancel is used")]
    pub status: Option<String>,
    #[schemars(description = "Queued message details when queue/cancel is used")]
    pub queued_message: Option<McpQueuedMessage>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpQueuedMessage {
    #[schemars(description = "The session id the message is queued for (UUID string)")]
    pub session_id: String,
    #[schemars(description = "The queued message content")]
    pub message: String,
    #[schemars(description = "Executor variant the queued message targets")]
    pub variant: Option<String>,
    #[schemars(description = "When the message was queued (RFC3339)")]
    pub queued_at: String,
}

impl McpQueuedMessage {
    fn from_queued_message(message: QueuedMessage) -> Self {
        Self {
            session_id: message.session_id.to_string(),
            message: message.data.message,
            variant: message.data.variant,
            queued_at: message.queued_at.to_rfc3339(),
        }
    }
}

impl FollowUpResponse {
    fn from_execution(
        session_id: Uuid,
        action: FollowUpAction,
        execution_process: ExecutionProcess,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            action,
            execution_process_id: Some(execution_process.id.to_string()),
            status: None,
            queued_message: None,
        }
    }

    fn from_status(session_id: Uuid, action: FollowUpAction, status: QueueStatus) -> Self {
        match status {
            QueueStatus::Empty => Self {
                session_id: session_id.to_string(),
                action,
                execution_process_id: None,
                status: Some("empty".to_string()),
                queued_message: None,
            },
            QueueStatus::Queued { message } => Self {
                session_id: session_id.to_string(),
                action,
                execution_process_id: None,
                status: Some("queued".to_string()),
                queued_message: Some(McpQueuedMessage::from_queued_message(message)),
            },
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptStatusRequest {
    #[schemars(
        description = "The attempt/workspace id to inspect (UUID string). This is required!"
    )]
    pub attempt_id: Uuid,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StopAttemptRequest {
    #[schemars(description = "The attempt/workspace id to stop (UUID string). This is required!")]
    pub attempt_id: Uuid,
    #[schemars(description = "If true, perform a hard stop (default: false).")]
    pub force: Option<bool>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct StopAttemptResponse {
    #[schemars(description = "The attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Whether a hard stop was requested")]
    pub force: bool,
}

#[derive(Debug, Deserialize)]
pub struct TailSessionMessagesRequest {
    pub session_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct TailSessionMessagesBySessionSchema {
    pub session_id: Uuid,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct TailSessionMessagesByAttemptSchema {
    pub attempt_id: Uuid,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum TailSessionMessagesRequestSchema {
    BySession(TailSessionMessagesBySessionSchema),
    ByAttempt(TailSessionMessagesByAttemptSchema),
}

impl schemars::JsonSchema for TailSessionMessagesRequest {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("TailSessionMessagesRequest")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        TailSessionMessagesRequestSchema::json_schema(generator)
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpSessionMessageTurn {
    #[schemars(description = "Monotonic transcript entry index for paging")]
    pub entry_index: i64,
    #[schemars(description = "Coding agent turn id (UUID string)")]
    pub turn_id: String,
    #[schemars(description = "User prompt for this turn, when available")]
    pub prompt: Option<String>,
    #[schemars(description = "Best-effort assistant summary/last message for this turn")]
    pub summary: Option<String>,
    #[schemars(description = "Turn created timestamp (RFC3339)")]
    pub created_at: String,
    #[schemars(description = "Turn updated timestamp (RFC3339)")]
    pub updated_at: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpSessionMessagesPage {
    #[schemars(description = "Transcript entries in chronological order (oldest→newest)")]
    pub entries: Vec<McpSessionMessageTurn>,
    #[schemars(description = "Cursor to request the next older page")]
    pub next_cursor: Option<i64>,
    #[schemars(description = "Whether older history exists beyond this page")]
    pub has_more: bool,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TailSessionMessagesResponse {
    #[schemars(description = "Resolved session id (UUID string)")]
    pub session_id: String,
    #[schemars(description = "Transcript history page")]
    pub page: McpSessionMessagesPage,
}

#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum McpAttemptState {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetAttemptStatusResponse {
    #[schemars(description = "The attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "The task id for this attempt (UUID string)")]
    pub task_id: String,
    #[schemars(description = "The workspace branch name for this attempt")]
    pub workspace_branch: String,
    #[schemars(description = "When the attempt was created (RFC3339)")]
    pub created_at: String,
    #[schemars(description = "When the attempt was last updated (RFC3339)")]
    pub updated_at: String,
    #[schemars(description = "Latest session id for this attempt (UUID string)")]
    pub latest_session_id: Option<String>,
    #[schemars(
        description = "Latest relevant execution process id for this attempt (UUID string)"
    )]
    pub latest_execution_process_id: Option<String>,
    #[schemars(description = "Coarse attempt lifecycle state (idle|running|completed|failed)")]
    pub state: McpAttemptState,
    #[schemars(description = "Best-effort last activity timestamp (RFC3339)")]
    pub last_activity_at: Option<String>,
    #[schemars(description = "Best-effort failure summary when state=failed")]
    pub failure_summary: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttemptLogChannel {
    Normalized,
    Raw,
}

#[derive(Debug, Deserialize)]
pub struct TailAttemptLogsRequest {
    pub attempt_id: Uuid,
    pub channel: Option<AttemptLogChannel>,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
    pub after_entry_index: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct TailAttemptLogsHistorySchema {
    pub attempt_id: Uuid,
    pub channel: Option<AttemptLogChannel>,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct TailAttemptLogsAfterSchema {
    pub attempt_id: Uuid,
    pub channel: Option<AttemptLogChannel>,
    pub limit: Option<usize>,
    pub after_entry_index: i64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
#[allow(dead_code)]
enum TailAttemptLogsRequestSchema {
    History(TailAttemptLogsHistorySchema),
    After(TailAttemptLogsAfterSchema),
}

impl schemars::JsonSchema for TailAttemptLogsRequest {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("TailAttemptLogsRequest")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        TailAttemptLogsRequestSchema::json_schema(generator)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct McpIndexedLogEntry {
    #[schemars(description = "Monotonic log entry index")]
    pub entry_index: i64,
    #[schemars(
        description = "Log entry payload (PatchType JSON). Treat as opaque JSON and render/inspect by `type`."
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

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct TailAttemptLogsResponse {
    #[schemars(description = "The attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(
        description = "Execution process id used for the tail (UUID string). Null when no relevant process exists."
    )]
    pub execution_process_id: Option<String>,
    #[schemars(description = "Channel used for this tail page")]
    pub channel: AttemptLogChannel,
    #[schemars(description = "Tail-first log history page")]
    pub page: McpLogHistoryPage,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptChangesRequest {
    #[schemars(description = "The attempt/workspace id (UUID string). This is required!")]
    pub attempt_id: Uuid,
    #[schemars(
        description = "If true, bypass diff preview guard thresholds and return a changed-file list (default: false)."
    )]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpAttemptChangesBlockedReason {
    SummaryFailed,
    ThresholdExceeded,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct McpAttemptChangesSummary {
    #[schemars(description = "Number of changed files")]
    pub file_count: usize,
    #[schemars(description = "Total added lines (best-effort)")]
    pub added: usize,
    #[schemars(description = "Total deleted lines (best-effort)")]
    pub deleted: usize,
    #[schemars(description = "Total bytes changed (best-effort)")]
    pub total_bytes: usize,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetAttemptChangesResponse {
    #[schemars(description = "The attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Diff summary across all repos in the attempt")]
    pub summary: McpAttemptChangesSummary,
    #[schemars(description = "Whether the changed-file list is blocked by guardrails")]
    pub blocked: bool,
    #[schemars(description = "Why the file list was blocked (when blocked=true)")]
    pub blocked_reason: Option<McpAttemptChangesBlockedReason>,
    #[schemars(description = "Stable code for recoverable blocks (present when blocked=true).")]
    pub code: Option<String>,
    #[schemars(
        description = "Whether retrying the same call may succeed without changing parameters (present when blocked=true)."
    )]
    pub retryable: Option<bool>,
    #[schemars(description = "Actionable next step when blocked=true.")]
    pub hint: Option<String>,
    #[schemars(
        description = "Changed file paths (repo-prefixed for multi-repo attempts). Empty when blocked."
    )]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpAttemptArtifactBlockedReason {
    PathOutsideWorkspace,
    SizeExceeded,
    TooManyPaths,
    SummaryFailed,
    ThresholdExceeded,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptFileRequest {
    #[schemars(description = "The attempt/workspace id (UUID string). This is required!")]
    pub attempt_id: Uuid,
    #[schemars(
        description = "Repo-prefixed path inside the attempt workspace (e.g. `my-repo/src/main.rs`)."
    )]
    pub path: String,
    #[schemars(description = "Optional byte offset to start reading from (default: 0).")]
    pub start: Option<u64>,
    #[schemars(
        description = "Optional max bytes to return (default: 65536; hard cap enforced)."
    )]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetAttemptFileResponse {
    #[schemars(description = "The attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Requested file path (repo-prefixed)")]
    pub path: String,
    #[schemars(description = "Whether access was blocked by guardrails")]
    pub blocked: bool,
    #[schemars(description = "Why access was blocked (when blocked=true)")]
    pub blocked_reason: Option<McpAttemptArtifactBlockedReason>,
    #[schemars(description = "Stable code for recoverable blocks (present when blocked=true).")]
    pub code: Option<String>,
    #[schemars(description = "Whether retrying without changing inputs may succeed.")]
    pub retryable: Option<bool>,
    #[schemars(description = "Actionable next step when blocked=true.")]
    pub hint: Option<String>,
    #[schemars(description = "Whether content was truncated to max_bytes")]
    pub truncated: bool,
    #[schemars(description = "Start offset used for this slice")]
    pub start: u64,
    #[schemars(description = "Bytes returned in content")]
    pub bytes: usize,
    #[schemars(description = "Total bytes in the file, when known")]
    pub total_bytes: Option<u64>,
    #[schemars(description = "File content slice (UTF-8, lossy). Null when blocked.")]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAttemptPatchRequest {
    #[schemars(description = "The attempt/workspace id (UUID string). This is required!")]
    pub attempt_id: Uuid,
    #[schemars(
        description = "Repo-prefixed file paths to include in the patch (e.g. `my-repo/src/lib.rs`)."
    )]
    pub paths: Vec<String>,
    #[schemars(
        description = "If true, bypass diff preview guard thresholds (still bounded by max_bytes and path limits)."
    )]
    pub force: Option<bool>,
    #[schemars(description = "Optional max bytes to return (default: 204800; hard cap enforced).")]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetAttemptPatchResponse {
    #[schemars(description = "The attempt/workspace id (UUID string)")]
    pub attempt_id: String,
    #[schemars(description = "Whether patch retrieval was blocked by guardrails")]
    pub blocked: bool,
    #[schemars(description = "Why patch retrieval was blocked (when blocked=true)")]
    pub blocked_reason: Option<McpAttemptArtifactBlockedReason>,
    #[schemars(description = "Stable code for recoverable blocks (present when blocked=true).")]
    pub code: Option<String>,
    #[schemars(description = "Whether retrying without changing inputs may succeed.")]
    pub retryable: Option<bool>,
    #[schemars(description = "Actionable next step when blocked=true.")]
    pub hint: Option<String>,
    #[schemars(description = "Whether the patch was truncated to max_bytes")]
    pub truncated: bool,
    #[schemars(description = "Bytes returned in patch")]
    pub bytes: usize,
    #[schemars(description = "Echo of requested paths")]
    pub paths: Vec<String>,
    #[schemars(description = "Unified diff patch (may be empty). Null when blocked.")]
    pub patch: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DeleteTaskResponse {
    #[schemars(description = "The deleted task id (UUID string)")]
    pub deleted_task_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTaskRequest {
    #[schemars(description = "The ID of the task to retrieve (UUID string)")]
    pub task_id: Uuid,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct GetTaskResponse {
    #[schemars(description = "Task details")]
    pub task: TaskDetails,
}

#[derive(Debug, Clone)]
pub struct TaskServer {
    client: reqwest::Client,
    base_url: String,
    tool_router: ToolRouter<TaskServer>,
    context: Option<McpContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct McpRepoContext {
    #[schemars(description = "The unique identifier of the repository (UUID string)")]
    pub repo_id: Uuid,
    #[schemars(description = "The name of the repository")]
    pub repo_name: String,
    #[schemars(description = "The target branch for this repository in this workspace")]
    pub target_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct McpContext {
    #[schemars(description = "The unique identifier of the project (UUID string)")]
    pub project_id: Uuid,
    #[schemars(description = "The name of the project")]
    pub project_name: String,
    #[schemars(description = "The unique identifier of the task (UUID string)")]
    pub task_id: Uuid,
    #[schemars(description = "The task title")]
    pub task_title: String,
    #[schemars(description = "Current task status (todo|inprogress|inreview|done|cancelled)")]
    pub task_status: String,
    #[schemars(description = "The attempt identifier for the active workspace (UUID string)")]
    pub attempt_id: Uuid,
    #[schemars(description = "The workspace branch for this attempt")]
    pub workspace_branch: String,
    #[schemars(
        description = "Repository info and target branches for each repo in this workspace"
    )]
    pub workspace_repos: Vec<McpRepoContext>,
}

impl TaskServer {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
            tool_router: Self::tool_router(),
            context: None,
        }
    }

    pub async fn init(mut self) -> Self {
        let context = self.fetch_context_at_startup().await;

        if context.is_none() {
            self.tool_router.map.remove("get_context");
            tracing::debug!("VK context not available, get_context tool will not be registered");
        } else {
            tracing::info!("VK context loaded, get_context tool available");
        }

        self.context = context;
        self
    }

    async fn fetch_context_at_startup(&self) -> Option<McpContext> {
        let current_dir = std::env::current_dir().ok()?;
        let canonical_path = current_dir.canonicalize().unwrap_or(current_dir);
        let normalized_path = utils::path::normalize_macos_private_alias(&canonical_path);

        let url = self.url("/api/containers/attempt-context");
        let query = ContainerQuery {
            container_ref: normalized_path.to_string_lossy().to_string(),
        };

        let response = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            self.client.get(&url).query(&query).send(),
        )
        .await
        .ok()?
        .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let api_response: ApiResponseEnvelope<WorkspaceContext> = response.json().await.ok()?;

        if !api_response.success {
            return None;
        }

        let ctx = api_response.data?;

        // Map RepoWithTargetBranch to McpRepoContext
        let workspace_repos: Vec<McpRepoContext> = ctx
            .workspace_repos
            .into_iter()
            .map(|rwb| McpRepoContext {
                repo_id: rwb.repo.id,
                repo_name: rwb.repo.name,
                target_branch: rwb.target_branch,
            })
            .collect();

        Some(McpContext {
            project_id: ctx.project.id,
            project_name: ctx.project.name,
            task_id: ctx.task.id,
            task_title: ctx.task.title,
            task_status: ctx.task.status.to_string(),
            attempt_id: ctx.workspace.id,
            workspace_branch: ctx.workspace.branch,
            workspace_repos,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponseEnvelope<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceWithSession {
    workspace: Workspace,
    session: Option<Session>,
}

#[derive(Debug, Serialize)]
struct TaskAttemptSummariesRequest {
    task_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
struct TaskAttemptSummaryEntry {
    task_id: Uuid,
    latest_attempt_id: Option<Uuid>,
    latest_workspace_branch: Option<String>,
    latest_session_id: Option<Uuid>,
    latest_session_executor: Option<String>,
}

impl TaskServer {
    fn success<T: Serialize>(data: &T) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(data)
                .unwrap_or_else(|_| "Failed to serialize response".to_string()),
        )]))
    }

    fn err_value(v: serde_json::Value) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::error(vec![Content::text(
            serde_json::to_string_pretty(&v)
                .unwrap_or_else(|_| "Failed to serialize error".to_string()),
        )]))
    }

    fn err_payload<S: Into<String>>(
        msg: S,
        details: Option<Value>,
        hint: Option<String>,
        code: Option<&'static str>,
        retryable: Option<bool>,
    ) -> Value {
        let mut v = serde_json::json!({"success": false, "error": msg.into()});
        if let Some(code) = code {
            v["code"] = serde_json::json!(code);
        }
        if let Some(details) = details {
            v["details"] = details;
        }
        if let Some(hint) = hint {
            v["hint"] = serde_json::json!(hint);
        }
        if let Some(retryable) = retryable {
            v["retryable"] = serde_json::json!(retryable);
        }
        v
    }

    fn err_with<S: Into<String>>(
        msg: S,
        details: Option<Value>,
        hint: Option<String>,
        code: Option<&'static str>,
        retryable: Option<bool>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::err_value(Self::err_payload(msg, details, hint, code, retryable))
    }

    fn truncate_body(body: &str, limit: usize) -> String {
        let mut chars = body.chars();
        let snippet: String = chars.by_ref().take(limit).collect();
        if chars.next().is_some() {
            format!("{snippet}... [truncated]")
        } else {
            snippet
        }
    }

    fn parse_api_message(body: &str) -> Option<String> {
        let value: Value = serde_json::from_str(body).ok()?;
        if let Some(msg) = value.get("message").and_then(|v| v.as_str()) {
            return Some(msg.to_string());
        }
        if let Some(msg) = value.get("error").and_then(|v| v.as_str()) {
            return Some(msg.to_string());
        }
        None
    }

    fn http_error_hint(status: StatusCode) -> Option<&'static str> {
        match status {
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                Some("Check tool inputs and IDs. Use list_* tools to fetch valid UUIDs.")
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Some(
                "Backend rejected the request. Ensure the backend is running locally and access is allowed.",
            ),
            StatusCode::NOT_FOUND => {
                Some("Resource not found. Verify IDs via list_* tools or get_context.")
            }
            StatusCode::CONFLICT => {
                Some("Conflict detected. Check for existing resources or adjust input values.")
            }
            StatusCode::TOO_MANY_REQUESTS => {
                Some("Rate limited. Wait a moment and retry the request.")
            }
            StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT => {
                Some("Backend is unhealthy or unavailable. Start/restart the backend and retry.")
            }
            _ => None,
        }
    }

    fn http_error(
        status: StatusCode,
        body: &str,
        method: &str,
        url: &str,
    ) -> Result<CallToolResult, ErrorData> {
        let mut details = serde_json::Map::new();
        details.insert("status".to_string(), serde_json::json!(status.as_u16()));
        if let Some(reason) = status.canonical_reason() {
            details.insert("status_text".to_string(), serde_json::json!(reason));
        }
        details.insert("method".to_string(), serde_json::json!(method));
        details.insert("url".to_string(), serde_json::json!(url));

        let body_snippet = Self::truncate_body(body, 1000);
        if !body_snippet.is_empty() {
            details.insert("body_snippet".to_string(), serde_json::json!(body_snippet));
        }
        if let Some(api_message) = Self::parse_api_message(body) {
            details.insert("api_message".to_string(), serde_json::json!(api_message));
        }

        let retryable = matches!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR
                | StatusCode::BAD_GATEWAY
                | StatusCode::SERVICE_UNAVAILABLE
                | StatusCode::GATEWAY_TIMEOUT
                | StatusCode::TOO_MANY_REQUESTS
        );

        Self::err_with(
            "VK API returned error status",
            Some(Value::Object(details)),
            Self::http_error_hint(status).map(|hint| hint.to_string()),
            Some("backend_http_error"),
            Some(retryable),
        )
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        rb: reqwest::RequestBuilder,
        method: &'static str,
        url: &str,
    ) -> Result<T, CallToolResult> {
        let resp = rb.send().await.map_err(|e| {
            Self::err_with(
                "Failed to connect to VK API",
                Some(serde_json::json!({
                    "error": e.to_string(),
                    "method": method,
                    "url": url,
                })),
                Some(
                    "Ensure the Vibe Kanban backend is running and reachable (VIBE_BACKEND_URL/BACKEND_PORT)."
                        .to_string(),
                ),
                Some("backend_unreachable"),
                Some(true),
            )
            .unwrap()
        })?;

        let status = resp.status();
        let resp_url = resp.url().to_string();
        let body = resp.text().await.map_err(|e| {
            Self::err_with(
                "Failed to read VK API response body",
                Some(serde_json::json!({
                    "error": e.to_string(),
                    "status": status.as_u16(),
                    "method": method,
                    "url": resp_url,
                })),
                Some("Retry the request or check backend logs.".to_string()),
                Some("backend_read_error"),
                Some(true),
            )
            .unwrap()
        })?;

        if !status.is_success() {
            return Err(Self::http_error(status, &body, method, &resp_url).unwrap());
        }

        let api_response: ApiResponseEnvelope<T> = serde_json::from_str(&body).map_err(|e| {
            Self::err_with(
                "Failed to parse VK API response",
                Some(serde_json::json!({
                    "error": e.to_string(),
                    "method": method,
                    "url": resp_url,
                    "body_snippet": Self::truncate_body(&body, 1000),
                })),
                Some("The backend returned invalid JSON. Check backend logs.".to_string()),
                Some("backend_invalid_response"),
                Some(false),
            )
            .unwrap()
        })?;

        if !api_response.success {
            let msg = api_response.message.as_deref().unwrap_or("Unknown error");
            return Err(Self::err_with(
                "VK API returned error",
                Some(serde_json::json!({
                    "message": msg,
                    "method": method,
                    "url": resp_url,
                })),
                Some("Check request inputs or call list_* tools to refresh IDs.".to_string()),
                Some("backend_error"),
                None,
            )
            .unwrap());
        }

        api_response.data.ok_or_else(|| {
            Self::err_with(
                "VK API response missing data field",
                Some(serde_json::json!({
                    "method": method,
                    "url": resp_url,
                })),
                Some("Check backend logs or retry.".to_string()),
                Some("backend_missing_data"),
                Some(false),
            )
            .unwrap()
        })
    }

    async fn send_ok(
        &self,
        rb: reqwest::RequestBuilder,
        method: &'static str,
        url: &str,
    ) -> Result<(), CallToolResult> {
        let resp = rb.send().await.map_err(|e| {
            Self::err_with(
                "Failed to connect to VK API",
                Some(serde_json::json!({
                    "error": e.to_string(),
                    "method": method,
                    "url": url,
                })),
                Some(
                    "Ensure the Vibe Kanban backend is running and reachable (VIBE_BACKEND_URL/BACKEND_PORT)."
                        .to_string(),
                ),
                Some("backend_unreachable"),
                Some(true),
            )
            .unwrap()
        })?;

        let status = resp.status();
        let resp_url = resp.url().to_string();
        let body = resp.text().await.map_err(|e| {
            Self::err_with(
                "Failed to read VK API response body",
                Some(serde_json::json!({
                    "error": e.to_string(),
                    "status": status.as_u16(),
                    "method": method,
                    "url": resp_url,
                })),
                Some("Retry the request or check backend logs.".to_string()),
                Some("backend_read_error"),
                Some(true),
            )
            .unwrap()
        })?;

        if !status.is_success() {
            return Err(Self::http_error(status, &body, method, &resp_url).unwrap());
        }

        let api_response: ApiResponseEnvelope<serde_json::Value> =
            serde_json::from_str(&body).map_err(|e| {
                Self::err_with(
                    "Failed to parse VK API response",
                    Some(serde_json::json!({
                        "error": e.to_string(),
                        "method": method,
                        "url": resp_url,
                        "body_snippet": Self::truncate_body(&body, 1000),
                    })),
                    Some("The backend returned invalid JSON. Check backend logs.".to_string()),
                    Some("backend_invalid_response"),
                    Some(false),
                )
                .unwrap()
            })?;

        if !api_response.success {
            let msg = api_response.message.as_deref().unwrap_or("Unknown error");
            return Err(Self::err_with(
                "VK API returned error",
                Some(serde_json::json!({
                    "message": msg,
                    "method": method,
                    "url": resp_url,
                })),
                Some("Check request inputs or call list_* tools to refresh IDs.".to_string()),
                Some("backend_error"),
                None,
            )
            .unwrap());
        }

        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    async fn resolve_session_id(
        &self,
        session_id: Option<Uuid>,
        attempt_id: Option<Uuid>,
        retry_tool: &'static str,
    ) -> Result<Uuid, CallToolResult> {
        match (session_id, attempt_id) {
            (Some(session_id), None) => return Ok(session_id),
            (None, Some(attempt_id)) => {
                let status_url = self.url(&format!("/api/task-attempts/{}/status", attempt_id));
                let status: ApiTaskAttemptStatusResponse = match self
                    .send_json(self.client.get(&status_url), "GET", &status_url)
                    .await
                {
                    Ok(status) => status,
                    Err(e) => return Err(e),
                };

                if let Some(latest_session_id) = status.latest_session_id {
                    return Ok(latest_session_id);
                }

                return Err(Self::err_with(
                    "No session exists for this attempt yet.",
                    Some(serde_json::json!({ "attempt_id": attempt_id.to_string() })),
                    Some(format!(
                        "Call get_attempt_status(attempt_id) and retry {retry_tool} once latest_session_id is non-null."
                    )),
                    Some(MCP_CODE_NO_SESSION_YET),
                    Some(true),
                )
                .unwrap());
            }
            (Some(session_id), Some(attempt_id)) => {
                return Err(Self::err_with(
                    "Provide exactly one target identifier (attempt_id OR session_id).",
                    Some(serde_json::json!({
                        "attempt_id": attempt_id.to_string(),
                        "session_id": session_id.to_string(),
                    })),
                    Some(
                        "Remove one of {attempt_id, session_id}. If you only have a task_id, call list_task_attempts first."
                            .to_string(),
                    ),
                    Some(MCP_CODE_AMBIGUOUS_TARGET),
                    Some(false),
                )
                .unwrap());
            }
            (None, None) => {
                return Err(Self::err_with(
                    "Missing target identifier (attempt_id OR session_id is required).",
                    None,
                    Some(
                        "Provide attempt_id from list_task_attempts, or session_id from get_context."
                            .to_string(),
                    ),
                    Some(MCP_CODE_AMBIGUOUS_TARGET),
                    Some(false),
                )
                .unwrap());
            }
        }
    }

    async fn fetch_attempts_with_latest_session(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<WorkspaceWithSession>, CallToolResult> {
        let url = self.url(&format!(
            "/api/task-attempts/with-latest-session?task_id={}",
            task_id
        ));

        match self.send_json(self.client.get(&url), "GET", &url).await {
            Ok(attempts) => Ok(attempts),
            Err(e) => Err(e),
        }
    }

    async fn fetch_task_attempt_summaries(
        &self,
        task_ids: Vec<Uuid>,
    ) -> Result<std::collections::HashMap<Uuid, TaskAttemptSummary>, CallToolResult> {
        if task_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let url = self.url("/api/task-attempts/latest-summaries");
        let payload = TaskAttemptSummariesRequest { task_ids };
        let summaries: Vec<TaskAttemptSummaryEntry> = match self
            .send_json(self.client.post(&url).json(&payload), "POST", &url)
            .await
        {
            Ok(summaries) => summaries,
            Err(e) => return Err(e),
        };

        let mut by_task = std::collections::HashMap::new();
        for summary in summaries {
            by_task.insert(
                summary.task_id,
                TaskAttemptSummary {
                    latest_attempt_id: summary.latest_attempt_id.map(|id| id.to_string()),
                    latest_workspace_branch: summary.latest_workspace_branch,
                    latest_session_id: summary.latest_session_id.map(|id| id.to_string()),
                    latest_session_executor: summary.latest_session_executor,
                },
            );
        }

        Ok(by_task)
    }

    fn summarize_attempts(attempts: &[WorkspaceWithSession]) -> TaskAttemptSummary {
        let latest = attempts
            .iter()
            .min_by(|a, b| Self::compare_attempts_newest_first(a, b));

        let Some(latest) = latest else {
            return TaskAttemptSummary::default();
        };

        TaskAttemptSummary {
            latest_attempt_id: Some(latest.workspace.id.to_string()),
            latest_workspace_branch: Some(latest.workspace.branch.clone()),
            latest_session_id: latest.session.as_ref().map(|s| s.id.to_string()),
            latest_session_executor: latest.session.as_ref().and_then(|s| s.executor.clone()),
        }
    }

    fn attempt_details_from(attempt: &WorkspaceWithSession) -> TaskAttemptDetails {
        TaskAttemptDetails {
            attempt_id: attempt.workspace.id.to_string(),
            workspace_branch: attempt.workspace.branch.clone(),
            created_at: attempt.workspace.created_at.to_rfc3339(),
            updated_at: attempt.workspace.updated_at.to_rfc3339(),
            latest_session_id: attempt.session.as_ref().map(|s| s.id.to_string()),
            latest_session_executor: attempt.session.as_ref().and_then(|s| s.executor.clone()),
        }
    }

    fn compare_attempts_newest_first(
        a: &WorkspaceWithSession,
        b: &WorkspaceWithSession,
    ) -> Ordering {
        let created_cmp = b.workspace.created_at.cmp(&a.workspace.created_at);
        if created_cmp == Ordering::Equal {
            a.workspace.id.cmp(&b.workspace.id)
        } else {
            created_cmp
        }
    }

    /// Expands @tagname references in text by replacing them with tag content.
    /// Returns the original text if expansion fails (e.g., network error).
    /// Unknown tags are left as-is (not expanded, not an error).
    async fn expand_tags(&self, text: &str) -> String {
        // Pattern matches @tagname where tagname is non-whitespace, non-@ characters
        let tag_pattern = match Regex::new(r"@([^\s@]+)") {
            Ok(re) => re,
            Err(_) => return text.to_string(),
        };

        // Find all unique tag names referenced in the text
        let tag_names: Vec<String> = tag_pattern
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if tag_names.is_empty() {
            return text.to_string();
        }

        // Fetch all tags from the API
        let url = self.url("/api/tags");
        let tags: Vec<Tag> = match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ApiResponseEnvelope<Vec<Tag>>>().await {
                    Ok(envelope) if envelope.success => envelope.data.unwrap_or_default(),
                    _ => return text.to_string(),
                }
            }
            _ => return text.to_string(),
        };

        // Build a map of tag_name -> content for quick lookup
        let tag_map: std::collections::HashMap<&str, &str> = tags
            .iter()
            .map(|t| (t.tag_name.as_str(), t.content.as_str()))
            .collect();

        // Replace each @tagname with its content (if found)
        let result = tag_pattern.replace_all(text, |caps: &regex::Captures| {
            let tag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            match tag_map.get(tag_name) {
                Some(content) => (*content).to_string(),
                None => caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string(),
            }
        });

        result.into_owned()
    }
}

#[tool_router]
impl TaskServer {
    #[tool(
        description = r#"Use when: You have an active VK workspace session and need its project/task/attempt IDs in one call.
Required: (none)
Optional: (none)
Next: list_tasks, get_attempt_status
Avoid: Calling this when no context is available (tool may not be registered)."#
    )]
    async fn get_context(&self) -> Result<CallToolResult, ErrorData> {
        // Context was fetched at startup and cached
        // This tool is only registered if context exists, so unwrap is safe
        let context = self.context.as_ref().expect("VK context should exist");
        TaskServer::success(context)
    }

    #[tool(
        description = r#"Use when: Create a new task/ticket in a project.
Required: project_id, title
Optional: description, request_id
Next: start_task_attempt
Avoid: Empty title; guessing project_id (use list_projects)."#
    )]
    async fn create_task(
        &self,
        Parameters(CreateTaskRequest {
            project_id,
            title,
            description,
            request_id,
        }): Parameters<CreateTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let title = title.trim();
        if title.is_empty() {
            return Self::err_with(
                "Title must not be empty.",
                None,
                Some("Provide a task title.".to_string()),
                Some("missing_required"),
                None,
            );
        }
        let title = title.to_string();

        // Expand @tagname references in description
        let expanded_description = match description {
            Some(desc) => Some(self.expand_tags(&desc).await),
            None => None,
        };

        let url = self.url("/api/tasks");
        let request_id = request_id.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let mut rb = self
            .client
            .post(&url)
            .json(&CreateTask::from_title_description(
                project_id,
                title,
                expanded_description,
            ));
        if let Some(request_id) = request_id.as_deref() {
            rb = rb.header(
                crate::routes::idempotency::IDEMPOTENCY_KEY_HEADER,
                request_id,
            );
        }

        let task: Task = match self.send_json(rb, "POST", &url).await {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        TaskServer::success(&CreateTaskResponse {
            task_id: task.id.to_string(),
        })
    }

    #[tool(
        description = r#"Use when: Discover project_id values.
Required: (none)
Optional: (none)
Next: list_tasks, list_repos
Avoid: Guessing UUIDs."#
    )]
    async fn list_projects(&self) -> Result<CallToolResult, ErrorData> {
        let url = self.url("/api/projects");
        let projects: Vec<Project> = match self.send_json(self.client.get(&url), "GET", &url).await
        {
            Ok(ps) => ps,
            Err(e) => return Ok(e),
        };

        let project_summaries: Vec<ProjectSummary> = projects
            .into_iter()
            .map(ProjectSummary::from_project)
            .collect();

        let response = ListProjectsResponse {
            count: project_summaries.len(),
            projects: project_summaries,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Get repo_id + names for a project (needed to start an attempt).
Required: project_id
Optional: (none)
Next: start_task_attempt
Avoid: Passing a task_id/attempt_id instead of project_id."#
    )]
    async fn list_repos(
        &self,
        Parameters(ListReposRequest { project_id }): Parameters<ListReposRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/projects/{}/repositories", project_id));
        let repos: Vec<Repo> = match self.send_json(self.client.get(&url), "GET", &url).await {
            Ok(rs) => rs,
            Err(e) => return Ok(e),
        };

        let repo_summaries: Vec<McpRepoSummary> = repos
            .into_iter()
            .map(|r| McpRepoSummary {
                id: r.id.to_string(),
                name: r.name,
            })
            .collect();

        let response = ListReposResponse {
            count: repo_summaries.len(),
            repos: repo_summaries,
            project_id: project_id.to_string(),
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Discover valid executor ids + variants for start_task_attempt (avoid hard-coding strings).
Required: (none)
Optional: (none)
Next: start_task_attempt
Avoid: Guessing executor names; passing DEFAULT as a variant (omit variant instead)."#
    )]
    async fn list_executors(&self) -> Result<CallToolResult, ErrorData> {
        let configs = executors::profile::ExecutorConfigs::get_cached();
        let mut executors = Vec::with_capacity(configs.executors.len());

        for (executor, config) in &configs.executors {
            let mut variants: Vec<String> = config
                .variant_names()
                .into_iter()
                .map(|name| name.to_string())
                .collect();
            variants.sort();

            let supports_mcp = config.get_default().map(|a| a.supports_mcp()).unwrap_or(false);

            executors.push(McpExecutorSummary {
                executor: executor.to_string(),
                variants,
                supports_mcp,
                default_variant: None,
            });
        }

        executors.sort_by(|a, b| a.executor.cmp(&b.executor));

        TaskServer::success(&ListExecutorsResponse {
            count: executors.len(),
            executors,
        })
    }

    #[tool(
        description = r#"Use when: List tasks in a project (includes latest attempt/session summary fields).
Required: project_id
Optional: status, limit
Next: get_task, start_task_attempt, list_task_attempts
Avoid: Using this as an attempt/session listing (use list_task_attempts)."#
    )]
    async fn list_tasks(
        &self,
        Parameters(ListTasksRequest {
            project_id,
            status,
            limit,
        }): Parameters<ListTasksRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let status_filter = if let Some(ref status_str) = status {
            let trimmed = status_str.trim();
            if trimmed.is_empty() {
                None
            } else {
                match TaskStatus::from_str(trimmed) {
                    Ok(s) => Some(s),
                    Err(_) => {
                        return Self::err_with(
                            "Invalid status filter",
                            Some(serde_json::json!({ "value": trimmed })),
                            Some(
                                "Valid values: todo, inprogress, inreview, done, cancelled."
                                    .to_string(),
                            ),
                            Some("invalid_argument"),
                            None,
                        );
                    }
                }
            }
        } else {
            None
        };

        let url = self.url(&format!("/api/tasks?project_id={}", project_id));
        let all_tasks: Vec<TaskWithAttemptStatus> =
            match self.send_json(self.client.get(&url), "GET", &url).await {
                Ok(t) => t,
                Err(e) => return Ok(e),
            };

        let task_limit = limit.unwrap_or(50).max(0) as usize;
        let filtered = all_tasks.into_iter().filter(|t| {
            if let Some(ref want) = status_filter {
                &t.status == want
            } else {
                true
            }
        });
        let limited: Vec<TaskWithAttemptStatus> = filtered.take(task_limit).collect();

        let task_ids: Vec<Uuid> = limited.iter().map(|task| task.id).collect();
        let summaries = match self.fetch_task_attempt_summaries(task_ids).await {
            Ok(summaries) => summaries,
            Err(e) => return Ok(e),
        };

        let mut task_summaries = Vec::with_capacity(limited.len());
        for task in limited {
            let attempt_summary = summaries.get(&task.id).cloned().unwrap_or_default();
            task_summaries.push(TaskSummary::from_task_with_status(task, attempt_summary));
        }

        let applied_status = status
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let response = ListTasksResponse {
            count: task_summaries.len(),
            tasks: task_summaries,
            project_id: project_id.to_string(),
            applied_filters: ListTasksFilters {
                status: applied_status,
                limit: task_limit as i32,
            },
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Get attempt_id values for a task and see latest_session_id/latest executor.
Required: task_id
Optional: (none)
Next: get_attempt_status, follow_up
Avoid: Using a project_id here (task_id is required)."#
    )]
    async fn list_task_attempts(
        &self,
        Parameters(ListTaskAttemptsRequest { task_id }): Parameters<ListTaskAttemptsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut attempts = match self.fetch_attempts_with_latest_session(task_id).await {
            Ok(attempts) => attempts,
            Err(e) => return Ok(e),
        };

        attempts.sort_by(Self::compare_attempts_newest_first);

        let attempt_details: Vec<TaskAttemptDetails> =
            attempts.iter().map(Self::attempt_details_from).collect();
        let summary = Self::summarize_attempts(&attempts);

        let response = ListTaskAttemptsResponse {
            task_id: task_id.to_string(),
            count: attempt_details.len(),
            attempts: attempt_details,
            latest_attempt_id: summary.latest_attempt_id,
            latest_session_id: summary.latest_session_id,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Create a new attempt/workspace for a task and start the executor (sets task status to inprogress).
Required: task_id, executor, repos
Optional: variant, request_id
Next: get_attempt_status (wait for latest_session_id), then follow_up(action=send)
Avoid: Calling update_task just to set inprogress; empty repos."#
    )]
    async fn start_task_attempt(
        &self,
        Parameters(StartTaskAttemptRequest {
            task_id,
            executor,
            variant,
            repos,
            request_id,
        }): Parameters<StartTaskAttemptRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if repos.is_empty() {
            return Self::err_with(
                "At least one repository must be specified.",
                None,
                Some("Call list_repos to get repo_id and target_branch.".to_string()),
                Some("missing_required"),
                None,
            );
        }

        let executor_trimmed = executor.trim();
        if executor_trimmed.is_empty() {
            return Self::err_with(
                "Executor must not be empty.",
                None,
                Some("Provide a supported executor (e.g., CLAUDE_CODE).".to_string()),
                Some("missing_required"),
                None,
            );
        }

        let normalized_executor = executor_trimmed.replace('-', "_").to_ascii_uppercase();
        let base_executor = match BaseCodingAgent::from_str(&normalized_executor) {
            Ok(exec) => exec,
            Err(_) => {
                return Self::err_with(
                    format!("Unknown executor '{executor_trimmed}'."),
                    Some(serde_json::json!({ "value": executor_trimmed })),
                    Some(format!(
                        "Valid executors: {}.",
                        CodingAgent::VARIANTS.join(", ")
                    )),
                    Some("invalid_argument"),
                    None,
                );
            }
        };

        let variant = variant.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        let executor_profile_id = ExecutorProfileId {
            executor: base_executor,
            variant,
        };

        let mut workspace_repos = Vec::with_capacity(repos.len());
        for (index, repo) in repos.into_iter().enumerate() {
            let target_branch = repo.target_branch.trim();
            if target_branch.is_empty() {
                return Self::err_with(
                    "Target branch must not be empty.",
                    Some(serde_json::json!({
                        "field": format!("repos[{index}].target_branch")
                    })),
                    Some("Provide a branch name like `main` or `master`.".to_string()),
                    Some("invalid_argument"),
                    None,
                );
            }

            workspace_repos.push(WorkspaceRepoInput {
                repo_id: repo.repo_id,
                target_branch: target_branch.to_string(),
            });
        }

        let payload = CreateTaskAttemptBody {
            task_id,
            executor_profile_id,
            repos: workspace_repos,
        };

        let url = self.url("/api/task-attempts");
        let request_id = request_id.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let mut rb = self.client.post(&url).json(&payload);
        if let Some(request_id) = request_id.as_deref() {
            rb = rb.header(
                crate::routes::idempotency::IDEMPOTENCY_KEY_HEADER,
                request_id,
            );
        }
        let workspace: Workspace = match self.send_json(rb, "POST", &url).await {
            Ok(workspace) => workspace,
            Err(e) => return Ok(e),
        };

        let response = StartTaskAttemptResponse {
            task_id: workspace.task_id.to_string(),
            attempt_id: workspace.id.to_string(),
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Send/queue/cancel a follow-up message to the coding agent for a specific session (or an attempt's latest session).
Required: action, exactly one of {attempt_id, session_id}; prompt is required for action=send|queue
Optional: variant, request_id
Next: get_attempt_status, tail_attempt_logs
Avoid: Providing both attempt_id and session_id; missing prompt for send/queue."#
    )]
    async fn follow_up(
        &self,
        Parameters(FollowUpRequest {
            session_id,
            attempt_id,
            prompt,
            action,
            variant,
            request_id,
        }): Parameters<FollowUpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if matches!(action, FollowUpAction::Cancel) && prompt.is_some() {
            return Self::err_with(
                "prompt is not allowed for action=cancel.",
                Some(serde_json::json!({ "field": "prompt" })),
                Some("Omit prompt when action=cancel.".to_string()),
                Some("invalid_argument"),
                Some(false),
            );
        }

        let session_id = match self
            .resolve_session_id(session_id, attempt_id, "follow_up")
            .await
        {
            Ok(session_id) => session_id,
            Err(e) => return Ok(e),
        };

        let prompt = match action {
            FollowUpAction::Send | FollowUpAction::Queue => {
                let prompt = prompt.unwrap_or_default();
                let trimmed = prompt.trim();
                if trimmed.is_empty() {
                    return Self::err_with(
                        "Prompt must not be empty.",
                        None,
                        Some("Provide a prompt string for send/queue actions.".to_string()),
                        Some("missing_required"),
                        None,
                    );
                }
                Some(trimmed.to_string())
            }
            FollowUpAction::Cancel => None,
        };

        let variant = variant.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let request_id = request_id.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        match action {
            FollowUpAction::Send => {
                #[derive(Serialize)]
                struct FollowUpPayload {
                    prompt: String,
                    variant: Option<String>,
                    retry_process_id: Option<Uuid>,
                    force_when_dirty: Option<bool>,
                    perform_git_reset: Option<bool>,
                }

                let payload = FollowUpPayload {
                    prompt: prompt.unwrap_or_default(),
                    variant,
                    retry_process_id: None,
                    force_when_dirty: None,
                    perform_git_reset: None,
                };

                let url = self.url(&format!("/api/sessions/{}/follow-up", session_id));
                let mut rb = self.client.post(&url).json(&payload);
                if let Some(request_id) = request_id.as_deref() {
                    rb = rb.header(
                        crate::routes::idempotency::IDEMPOTENCY_KEY_HEADER,
                        request_id,
                    );
                }
                let execution_process: ExecutionProcess =
                    match self.send_json(rb, "POST", &url).await {
                        Ok(process) => process,
                        Err(e) => return Ok(e),
                    };

                TaskServer::success(&FollowUpResponse::from_execution(
                    session_id,
                    action,
                    execution_process,
                ))
            }
            FollowUpAction::Queue => {
                #[derive(Serialize)]
                struct QueuePayload {
                    message: String,
                    variant: Option<String>,
                }

                let payload = QueuePayload {
                    message: prompt.unwrap_or_default(),
                    variant,
                };

                let url = self.url(&format!("/api/sessions/{}/queue", session_id));
                let mut rb = self.client.post(&url).json(&payload);
                if let Some(request_id) = request_id.as_deref() {
                    rb = rb.header(
                        crate::routes::idempotency::IDEMPOTENCY_KEY_HEADER,
                        request_id,
                    );
                }
                let status: QueueStatus = match self.send_json(rb, "POST", &url).await {
                    Ok(status) => status,
                    Err(e) => return Ok(e),
                };

                TaskServer::success(&FollowUpResponse::from_status(session_id, action, status))
            }
            FollowUpAction::Cancel => {
                let url = self.url(&format!("/api/sessions/{}/queue", session_id));
                let status: QueueStatus = match self
                    .send_json(self.client.delete(&url), "DELETE", &url)
                    .await
                {
                    Ok(status) => status,
                    Err(e) => return Ok(e),
                };

                TaskServer::success(&FollowUpResponse::from_status(session_id, action, status))
            }
        }
    }

    #[tool(
        description = r#"Use when: Check attempt state and discover latest_session_id / latest_execution_process_id.
Required: attempt_id
Optional: (none)
Next: tail_attempt_logs, get_attempt_changes, follow_up
Avoid: Passing task_id where attempt_id is required."#
    )]
    async fn get_attempt_status(
        &self,
        Parameters(GetAttemptStatusRequest { attempt_id }): Parameters<GetAttemptStatusRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/task-attempts/{}/status", attempt_id));
        let status: ApiTaskAttemptStatusResponse =
            match self.send_json(self.client.get(&url), "GET", &url).await {
                Ok(status) => status,
                Err(e) => return Ok(e),
            };

        let state = match status.state {
            ApiAttemptState::Idle => McpAttemptState::Idle,
            ApiAttemptState::Running => McpAttemptState::Running,
            ApiAttemptState::Completed => McpAttemptState::Completed,
            ApiAttemptState::Failed => McpAttemptState::Failed,
        };

        let response = GetAttemptStatusResponse {
            attempt_id: status.attempt_id.to_string(),
            task_id: status.task_id.to_string(),
            workspace_branch: status.workspace_branch,
            created_at: status.created_at.to_rfc3339(),
            updated_at: status.updated_at.to_rfc3339(),
            latest_session_id: status.latest_session_id.map(|id| id.to_string()),
            latest_execution_process_id: status
                .latest_execution_process_id
                .map(|id| id.to_string()),
            state,
            last_activity_at: status.last_activity_at.map(|at| at.to_rfc3339()),
            failure_summary: status.failure_summary,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Restore session transcript context (prompt + summary per turn) for an attempt's latest session.
Required: exactly one of {attempt_id, session_id}
Optional: limit, cursor
Next: follow_up(action=send|queue), get_attempt_status
Avoid: Passing both attempt_id and session_id; expecting raw tool logs (use tail_attempt_logs)."#
    )]
    async fn tail_session_messages(
        &self,
        Parameters(TailSessionMessagesRequest {
            session_id,
            attempt_id,
            limit,
            cursor,
        }): Parameters<TailSessionMessagesRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let session_id = match self
            .resolve_session_id(session_id, attempt_id, "tail_session_messages")
            .await
        {
            Ok(session_id) => session_id,
            Err(e) => return Ok(e),
        };

        let url = self.url(&format!("/api/sessions/{}/messages", session_id));
        let mut rb = self.client.get(&url);
        if let Some(limit) = limit {
            let capped = limit.clamp(1, 200);
            rb = rb.query(&[("limit", capped)]);
        }
        if let Some(cursor) = cursor {
            rb = rb.query(&[("cursor", cursor)]);
        }

        let page: crate::routes::sessions::SessionMessagesPage =
            match self.send_json(rb, "GET", &url).await {
                Ok(page) => page,
                Err(e) => return Ok(e),
            };

        let entries = page
            .entries
            .into_iter()
            .map(|entry| McpSessionMessageTurn {
                entry_index: entry.entry_index,
                turn_id: entry.turn_id.to_string(),
                prompt: entry.prompt,
                summary: entry.summary,
                created_at: entry.created_at.to_rfc3339(),
                updated_at: entry.updated_at.to_rfc3339(),
            })
            .collect::<Vec<_>>();

        TaskServer::success(&TailSessionMessagesResponse {
            session_id: session_id.to_string(),
            page: McpSessionMessagesPage {
                entries,
                next_cursor: page.next_cursor,
                has_more: page.has_more,
            },
        })
    }

    #[tool(
        description = r#"Use when: Stop a running attempt's non-dev-server execution (kill runaway agent runs).
Required: attempt_id
Optional: force
Next: get_attempt_status, tail_attempt_logs
Avoid: Using follow_up(action=cancel) for stopping execution; expecting this to stop dev servers."#
    )]
    async fn stop_attempt(
        &self,
        Parameters(StopAttemptRequest { attempt_id, force }): Parameters<StopAttemptRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        #[derive(Serialize)]
        struct StopAttemptQuery {
            force: Option<bool>,
        }

        let url = self.url(&format!("/api/task-attempts/{}/stop", attempt_id));
        let query = StopAttemptQuery { force };
        if let Err(e) = self
            .send_ok(self.client.post(&url).query(&query), "POST", &url)
            .await
        {
            return Ok(e);
        }

        TaskServer::success(&StopAttemptResponse {
            attempt_id: attempt_id.to_string(),
            force: force.unwrap_or(false),
        })
    }

    #[tool(
        description = r#"Use when: Fetch recent attempt logs (pull mode) after get_attempt_status.
Required: attempt_id
Optional: channel, limit, cursor, after_entry_index
Next: get_attempt_status (until not running), get_attempt_changes
Avoid: Mixing cursor with any after_* tail parameter; using raw logs unless needed."#
    )]
    async fn tail_attempt_logs(
        &self,
        Parameters(TailAttemptLogsRequest {
            attempt_id,
            channel,
            limit,
            cursor,
            after_entry_index,
        }): Parameters<TailAttemptLogsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let channel = channel.unwrap_or(AttemptLogChannel::Normalized);

        if cursor.is_some() && after_entry_index.is_some() {
            return Self::err_with(
                "cursor and after_entry_index are mutually exclusive.",
                Some(serde_json::json!({
                    "cursor": cursor,
                    "after_entry_index": after_entry_index,
                })),
                Some(
                    "Use cursor to page older history; use after_entry_index to fetch only new entries."
                        .to_string(),
                ),
                Some(MCP_CODE_MIXED_PAGINATION),
                Some(false),
            );
        }

        let status_url = self.url(&format!("/api/task-attempts/{}/status", attempt_id));
        let status: ApiTaskAttemptStatusResponse = match self
            .send_json(self.client.get(&status_url), "GET", &status_url)
            .await
        {
            Ok(status) => status,
            Err(e) => return Ok(e),
        };

        let Some(exec_id) = status.latest_execution_process_id else {
            let response = TailAttemptLogsResponse {
                attempt_id: attempt_id.to_string(),
                execution_process_id: None,
                channel,
                page: McpLogHistoryPage {
                    entries: Vec::new(),
                    next_cursor: None,
                    has_more: false,
                    history_truncated: false,
                },
            };
            return TaskServer::success(&response);
        };

        let logs_path = match channel {
            AttemptLogChannel::Normalized => {
                format!("/api/execution-processes/{}/normalized-logs/v2", exec_id)
            }
            AttemptLogChannel::Raw => {
                format!("/api/execution-processes/{}/raw-logs/v2", exec_id)
            }
        };
        let url = self.url(&logs_path);

        let mut rb = self.client.get(&url);
        if let Some(limit) = limit {
            let capped = limit.clamp(1, 1000);
            rb = rb.query(&[("limit", capped)]);
        }
        if let Some(cursor) = cursor {
            rb = rb.query(&[("cursor", cursor)]);
        }

        let page: McpLogHistoryPage = match self.send_json(rb, "GET", &url).await {
            Ok(page) => page,
            Err(e) => return Ok(e),
        };

        let mut page = page;
        if let Some(after_entry_index) = after_entry_index {
            page.entries
                .retain(|entry| entry.entry_index > after_entry_index);
        }

        let response = TailAttemptLogsResponse {
            attempt_id: attempt_id.to_string(),
            execution_process_id: Some(exec_id.to_string()),
            channel,
            page,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Get a diff summary and (if allowed) a changed-file list for an attempt.
Required: attempt_id
Optional: force
Next: get_attempt_patch, follow_up
Avoid: Assuming files will be returned when blocked=true; using force unless you accept larger output."#
    )]
    async fn get_attempt_changes(
        &self,
        Parameters(GetAttemptChangesRequest { attempt_id, force }): Parameters<
            GetAttemptChangesRequest,
        >,
    ) -> Result<CallToolResult, ErrorData> {
        let force = force.unwrap_or(false);
        let url = self.url(&format!("/api/task-attempts/{}/changes", attempt_id));
        let mut rb = self.client.get(&url);
        if force {
            rb = rb.query(&[("force", true)]);
        }

        let changes: ApiTaskAttemptChangesResponse = match self.send_json(rb, "GET", &url).await {
            Ok(changes) => changes,
            Err(e) => return Ok(e),
        };

        let blocked_reason = match changes.blocked_reason {
            Some(ApiAttemptChangesBlockedReason::SummaryFailed) => {
                Some(McpAttemptChangesBlockedReason::SummaryFailed)
            }
            Some(ApiAttemptChangesBlockedReason::ThresholdExceeded) => {
                Some(McpAttemptChangesBlockedReason::ThresholdExceeded)
            }
            None => None,
        };

        let (code, retryable, hint) = if changes.blocked && !force {
            (
                Some(MCP_CODE_BLOCKED_GUARDRAILS.to_string()),
                Some(false),
                Some(
                    "Changed-file list blocked by diff preview guardrails. Retry with force=true if you accept a larger file list."
                        .to_string(),
                ),
            )
        } else {
            (None, None, None)
        };

        let response = GetAttemptChangesResponse {
            attempt_id: attempt_id.to_string(),
            summary: McpAttemptChangesSummary {
                file_count: changes.summary.file_count,
                added: changes.summary.added,
                deleted: changes.summary.deleted,
                total_bytes: changes.summary.total_bytes,
            },
            blocked: changes.blocked,
            blocked_reason,
            code,
            retryable,
            hint,
            files: changes.files,
        };

        TaskServer::success(&response)
    }

    #[tool(
        description = r#"Use when: Read a bounded slice of a file inside an attempt workspace (code/config/artifacts).
Required: attempt_id, path
Optional: start, max_bytes
Next: get_attempt_patch, follow_up
Avoid: Absolute paths or `..` traversal; requesting huge files without narrowing max_bytes."#
    )]
    async fn get_attempt_file(
        &self,
        Parameters(GetAttemptFileRequest {
            attempt_id,
            path,
            start,
            max_bytes,
        }): Parameters<GetAttemptFileRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Self::err_with(
                "path must not be empty.",
                Some(serde_json::json!({ "field": "path" })),
                Some("Provide a repo-prefixed path like `my-repo/src/main.rs`.".to_string()),
                Some("missing_required"),
                None,
            );
        }

        let url = self.url(&format!("/api/task-attempts/{}/file", attempt_id));
        let mut rb = self.client.get(&url).query(&[("path", trimmed)]);
        if let Some(start) = start {
            rb = rb.query(&[("start", start)]);
        }
        if let Some(max_bytes) = max_bytes {
            let capped = max_bytes.max(1);
            rb = rb.query(&[("max_bytes", capped)]);
        }

        let file: ApiAttemptFileResponse = match self.send_json(rb, "GET", &url).await {
            Ok(file) => file,
            Err(e) => return Ok(e),
        };

        let blocked_reason = match file.blocked_reason {
            Some(ApiAttemptArtifactBlockedReason::PathOutsideWorkspace) => {
                Some(McpAttemptArtifactBlockedReason::PathOutsideWorkspace)
            }
            Some(ApiAttemptArtifactBlockedReason::SizeExceeded) => {
                Some(McpAttemptArtifactBlockedReason::SizeExceeded)
            }
            Some(ApiAttemptArtifactBlockedReason::TooManyPaths) => {
                Some(McpAttemptArtifactBlockedReason::TooManyPaths)
            }
            Some(ApiAttemptArtifactBlockedReason::SummaryFailed) => {
                Some(McpAttemptArtifactBlockedReason::SummaryFailed)
            }
            Some(ApiAttemptArtifactBlockedReason::ThresholdExceeded) => {
                Some(McpAttemptArtifactBlockedReason::ThresholdExceeded)
            }
            None => None,
        };

        let (code, retryable, hint) = if file.blocked {
            let hint = match blocked_reason {
                Some(McpAttemptArtifactBlockedReason::PathOutsideWorkspace) => {
                    "Blocked: path is outside the attempt workspace. Use get_attempt_changes(attempt_id) to get valid repo-prefixed paths, and avoid absolute paths / '..'."
                        .to_string()
                }
                Some(McpAttemptArtifactBlockedReason::SizeExceeded) => {
                    "Blocked: requested size exceeds limits. Reduce max_bytes and/or adjust start to read a smaller slice."
                        .to_string()
                }
                _ => "Blocked by guardrails. Narrow the request.".to_string(),
            };
            (
                Some(MCP_CODE_BLOCKED_GUARDRAILS.to_string()),
                Some(false),
                Some(hint),
            )
        } else {
            (None, None, None)
        };

        TaskServer::success(&GetAttemptFileResponse {
            attempt_id: attempt_id.to_string(),
            path: file.path,
            blocked: file.blocked,
            blocked_reason,
            code,
            retryable,
            hint,
            truncated: file.truncated,
            start: file.start,
            bytes: file.bytes,
            total_bytes: file.total_bytes,
            content: file.content,
        })
    }

    #[tool(
        description = r#"Use when: Retrieve a bounded unified diff patch for selected files in an attempt (for review/apply).
Required: attempt_id, paths
Optional: force, max_bytes
Next: follow_up, stop_attempt
Avoid: Passing too many paths; expecting a full repo patch by default; forgetting force=true when diff guardrails block."#
    )]
    async fn get_attempt_patch(
        &self,
        Parameters(GetAttemptPatchRequest {
            attempt_id,
            paths,
            force,
            max_bytes,
        }): Parameters<GetAttemptPatchRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        if paths.is_empty() {
            return Self::err_with(
                "paths must not be empty.",
                Some(serde_json::json!({ "field": "paths" })),
                Some(
                    "Provide repo-prefixed paths (e.g. from get_attempt_changes.files)."
                        .to_string(),
                ),
                Some("missing_required"),
                None,
            );
        }

        #[derive(Serialize)]
        struct ApiPatchRequest {
            paths: Vec<String>,
            force: bool,
            max_bytes: Option<usize>,
        }

        let force = force.unwrap_or(false);
        let url = self.url(&format!("/api/task-attempts/{}/patch", attempt_id));
        let request = ApiPatchRequest {
            paths,
            force,
            max_bytes,
        };

        let patch: ApiAttemptPatchResponse = match self
            .send_json(self.client.post(&url).json(&request), "POST", &url)
            .await
        {
            Ok(patch) => patch,
            Err(e) => return Ok(e),
        };

        let blocked_reason = match patch.blocked_reason {
            Some(ApiAttemptArtifactBlockedReason::PathOutsideWorkspace) => {
                Some(McpAttemptArtifactBlockedReason::PathOutsideWorkspace)
            }
            Some(ApiAttemptArtifactBlockedReason::SizeExceeded) => {
                Some(McpAttemptArtifactBlockedReason::SizeExceeded)
            }
            Some(ApiAttemptArtifactBlockedReason::TooManyPaths) => {
                Some(McpAttemptArtifactBlockedReason::TooManyPaths)
            }
            Some(ApiAttemptArtifactBlockedReason::SummaryFailed) => {
                Some(McpAttemptArtifactBlockedReason::SummaryFailed)
            }
            Some(ApiAttemptArtifactBlockedReason::ThresholdExceeded) => {
                Some(McpAttemptArtifactBlockedReason::ThresholdExceeded)
            }
            None => None,
        };

        let (code, retryable, hint) = if patch.blocked {
            let hint = match blocked_reason {
                Some(McpAttemptArtifactBlockedReason::ThresholdExceeded)
                | Some(McpAttemptArtifactBlockedReason::SummaryFailed) if !force => {
                    "Patch blocked by diff preview guardrails. Retry with force=true and a narrow paths list."
                        .to_string()
                }
                Some(McpAttemptArtifactBlockedReason::TooManyPaths) => {
                    "Patch blocked: too many paths. Reduce paths to a small set of specific files."
                        .to_string()
                }
                Some(McpAttemptArtifactBlockedReason::SizeExceeded) => {
                    "Patch blocked: size exceeded. Reduce max_bytes and/or narrow paths."
                        .to_string()
                }
                Some(McpAttemptArtifactBlockedReason::PathOutsideWorkspace) => {
                    "Patch blocked: path outside workspace. Use get_attempt_changes(attempt_id) to get valid repo-prefixed paths."
                        .to_string()
                }
                _ => "Patch blocked by guardrails. Narrow the request.".to_string(),
            };
            (
                Some(MCP_CODE_BLOCKED_GUARDRAILS.to_string()),
                Some(false),
                Some(hint),
            )
        } else if patch.truncated {
            (
                Some(MCP_CODE_BLOCKED_GUARDRAILS.to_string()),
                Some(false),
                Some(
                    "Patch truncated by max_bytes. Narrow paths or increase max_bytes (within limits) to retrieve more."
                        .to_string(),
                ),
            )
        } else {
            (None, None, None)
        };

        TaskServer::success(&GetAttemptPatchResponse {
            attempt_id: attempt_id.to_string(),
            blocked: patch.blocked,
            blocked_reason,
            code,
            retryable,
            hint,
            truncated: patch.truncated,
            bytes: patch.bytes,
            paths: patch.paths,
            patch: patch.patch,
        })
    }

    #[tool(
        description = r#"Use when: Update a task's title/description/status.
Required: task_id
Optional: title, description, status
Next: get_task, list_tasks
Avoid: Calling this just to set status=inprogress (start_task_attempt already does that)."#
    )]
    async fn update_task(
        &self,
        Parameters(UpdateTaskRequest {
            task_id,
            title,
            description,
            status,
        }): Parameters<UpdateTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let status = if let Some(ref status_str) = status {
            let trimmed = status_str.trim();
            if trimmed.is_empty() {
                None
            } else {
                match TaskStatus::from_str(trimmed) {
                    Ok(s) => Some(s),
                    Err(_) => {
                        return Self::err_with(
                            "Invalid status value",
                            Some(serde_json::json!({ "value": trimmed })),
                            Some(
                                "Valid values: todo, inprogress, inreview, done, cancelled."
                                    .to_string(),
                            ),
                            Some("invalid_argument"),
                            None,
                        );
                    }
                }
            }
        } else {
            None
        };

        // Expand @tagname references in description
        let expanded_description = match description {
            Some(desc) => Some(self.expand_tags(&desc).await),
            None => None,
        };

        let payload = UpdateTask {
            title,
            description: expanded_description,
            status,
            parent_workspace_id: None,
            image_ids: None,
        };
        let url = self.url(&format!("/api/tasks/{}", task_id));
        let updated_task: Task = match self
            .send_json(self.client.put(&url).json(&payload), "PUT", &url)
            .await
        {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let details = TaskDetails::from_task(updated_task);
        let repsonse = UpdateTaskResponse { task: details };
        TaskServer::success(&repsonse)
    }

    #[tool(
        description = r#"Use when: Permanently delete a task/ticket.
Required: task_id
Optional: (none)
Next: list_tasks
Avoid: Deleting the wrong task (confirm with get_task first)."#
    )]
    async fn delete_task(
        &self,
        Parameters(DeleteTaskRequest { task_id }): Parameters<DeleteTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/tasks/{}", task_id));
        if let Err(e) = self
            .send_ok(self.client.delete(&url), "DELETE", &url)
            .await
        {
            return Ok(e);
        }

        let repsonse = DeleteTaskResponse {
            deleted_task_id: Some(task_id.to_string()),
        };

        TaskServer::success(&repsonse)
    }

    #[tool(
        description = r#"Use when: Fetch full task details (title/description/status).
Required: task_id
Optional: (none)
Next: update_task, start_task_attempt
Avoid: Expecting attempt/session info here (use list_tasks/list_task_attempts)."#
    )]
    async fn get_task(
        &self,
        Parameters(GetTaskRequest { task_id }): Parameters<GetTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/tasks/{}", task_id));
        let task: Task = match self.send_json(self.client.get(&url), "GET", &url).await {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let details = TaskDetails::from_task(task);
        let response = GetTaskResponse { task: details };

        TaskServer::success(&response)
    }
}

#[tool_handler]
impl ServerHandler for TaskServer {
    fn get_info(&self) -> ServerInfo {
        let mut instruction = "A task and project management server. Use list tools to discover ids, then perform create/update/start/follow-up actions. start_task_attempt automatically sets the task to inprogress; do not call update_task just to set inprogress. After start_task_attempt, use follow_up to send the prompt to the attempt. For attempt observability (closed-loop), use: get_attempt_status → tail_attempt_logs (cursor/limit, default normalized) → get_attempt_changes (diff summary + changed files; may be blocked unless force=true). TOOLS: 'list_projects', 'list_repos', 'list_tasks', 'list_task_attempts', 'get_task', 'create_task', 'update_task', 'delete_task', 'start_task_attempt', 'follow_up', 'get_attempt_status', 'tail_attempt_logs', 'get_attempt_changes'. Use `list_tasks` to get task ids and latest attempt/session summaries. Use `list_task_attempts` to inspect attempts and get attempt_id/session_id for follow-up. The `follow_up` tool accepts either attempt_id (preferred) or session_id. attempt_id refers to the task attempt workspace id. Tool errors are returned as JSON with fields like error, hint, code, and retryable; invalid parameter errors include JSON-RPC error data with path/hint.".to_string();
        if self.context.is_some() {
            let context_instruction = "Use 'get_context' to fetch project/task/attempt metadata for the active Vibe Kanban workspace session when available.";
            instruction = format!("{} {}", context_instruction, instruction);
        }

        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "vibe-kanban".to_string(),
                title: Some("Vibe Kanban MCP Server".to_string()),
                version: "1.0.0".to_string(),
                icons: Some(vec![Icon {
                    src: "https://raw.githubusercontent.com/StrayDragon/vibe-kanban/main/frontend/public/vibe-kanban-logo.svg".to_string(),
                    mime_type: Some("image/svg+xml".to_string()),
                    sizes: Some(vec!["any".to_string()]),
                }]),
                website_url: Some("https://www.vibekanban.com".to_string()),
            },
            instructions: Some(instruction),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum;
    use db::models::{
        coding_agent_turn::{CodingAgentTurn, CreateCodingAgentTurn},
        execution_process::{CreateExecutionProcess, ExecutionProcess, ExecutionProcessRunReason},
        execution_process_log_entries::ExecutionProcessLogEntry,
        project::{CreateProject, Project},
        project_repo::ProjectRepo,
        repo::Repo,
        session::{CreateSession, Session},
        task::{CreateTask, Task},
        workspace::{CreateWorkspace, Workspace},
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    };
    use deployment::Deployment;
    use executors::{
        actions::{
            ExecutorAction, ExecutorActionType,
            script::{ScriptContext, ScriptRequest, ScriptRequestLanguage},
        },
        logs::{NormalizedEntry, NormalizedEntryType, utils::patch::PatchType},
    };
    use local_deployment::container::LocalContainerService;
    use rmcp::ServerHandler as _;
    use services::services::{
        config::DiffPreviewGuardPreset,
        git::GitService,
        workspace_manager::{RepoWorkspaceInput, WorkspaceManager},
    };
    use utils::log_entries::LogEntryChannel;
    use uuid::Uuid;

    use super::*;
    use crate::{DeploymentImpl, http, test_support::TestEnvGuard};

    fn tool_json(result: CallToolResult) -> serde_json::Value {
        assert_eq!(result.is_error, Some(false));
        let text = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|text| text.text.as_str())
            .unwrap_or("");
        serde_json::from_str(text).expect("tool should return valid JSON text")
    }

    fn tool_error_json(result: CallToolResult) -> serde_json::Value {
        assert_eq!(result.is_error, Some(true));
        let text = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|text| text.text.as_str())
            .unwrap_or("");
        serde_json::from_str(text).expect("tool should return valid JSON text")
    }

    async fn start_backend(
        deployment: DeploymentImpl,
    ) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let app = http::router(deployment);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, handle)
    }

    #[test]
    fn tool_router_includes_closed_loop_tools() {
        let server = TaskServer::new("http://example.com");
        assert!(server.tool_router.map.contains_key("get_attempt_status"));
        assert!(server.tool_router.map.contains_key("tail_attempt_logs"));
        assert!(server.tool_router.map.contains_key("get_attempt_changes"));

        let info = server.get_info();
        let instructions = info.instructions.unwrap_or_default();
        assert!(instructions.contains("get_attempt_status"));
        assert!(instructions.contains("tail_attempt_logs"));
        assert!(instructions.contains("get_attempt_changes"));
    }

    #[test]
    fn follow_up_description_uses_guidance_template() {
        let server = TaskServer::new("http://example.com");
        let tool = server
            .tool_router
            .map
            .get("follow_up")
            .expect("follow_up tool should be registered");
        let desc = tool.attr.description.as_ref().map(|d| d.as_ref()).unwrap_or("");
        assert!(desc.contains("Use when:"));
        assert!(desc.contains("Required:"));
        assert!(desc.contains("Next:"));
        assert!(desc.contains("Avoid:"));
    }

    #[test]
    fn follow_up_schema_enforces_action_specific_requirements() {
        fn deref_schema<'a>(
            schema: &'a serde_json::Value,
            defs: &'a serde_json::Map<String, serde_json::Value>,
        ) -> &'a serde_json::Value {
            let Some(schema_ref) = schema.get("$ref").and_then(|v| v.as_str()) else {
                return schema;
            };
            let name = schema_ref
                .strip_prefix("#/$defs/")
                .or_else(|| schema_ref.strip_prefix("#/definitions/"));
            let Some(name) = name else { return schema };
            defs.get(name).unwrap_or(schema)
        }

        let server = TaskServer::new("http://example.com");
        let tool = server
            .tool_router
            .map
            .get("follow_up")
            .expect("follow_up tool should be registered");

        let schema = serde_json::Value::Object(tool.attr.input_schema.as_ref().clone());
        let defs = schema
            .get("$defs")
            .or_else(|| schema.get("definitions"))
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let root = deref_schema(&schema, &defs);
        let variants = root
            .get("oneOf")
            .or_else(|| root.get("anyOf"))
            .and_then(|v| v.as_array())
            .or_else(|| {
                root.get("allOf").and_then(|v| v.as_array()).and_then(|all_of| {
                    all_of.iter().find_map(|item| {
                        let item = deref_schema(item, &defs);
                        item.get("oneOf")
                            .or_else(|| item.get("anyOf"))
                            .and_then(|v| v.as_array())
                    })
                })
            })
            .expect("follow_up schema should include oneOf/anyOf variants");

        let mut saw_send = false;
        let mut saw_cancel = false;
        for variant in variants {
            let variant = deref_schema(variant, &defs);
            let props = variant
                .get("properties")
                .and_then(|v| v.as_object())
                .expect("variant should have properties");
            let action_schema = props.get("action").expect("action schema missing");
            let action_schema = deref_schema(action_schema, &defs);
            let action = action_schema
                .get("const")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    action_schema
                        .get("enum")
                        .and_then(|v| v.as_array())
                        .and_then(|v| v.first())
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("");

            let required = variant
                .get("required")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if action == "send" {
                saw_send = true;
                assert!(required.iter().any(|v| v.as_str() == Some("prompt")));
                assert!(required.iter().any(|v| v.as_str() == Some("attempt_id"))
                    || required.iter().any(|v| v.as_str() == Some("session_id")));
            }

            if action == "cancel" {
                saw_cancel = true;
                assert!(!required.iter().any(|v| v.as_str() == Some("prompt")));
                assert!(!props.contains_key("prompt"));
            }
        }

        assert!(saw_send, "schema should include a send branch");
        assert!(saw_cancel, "schema should include a cancel branch");
    }

    #[tokio::test]
    async fn follow_up_rejects_ambiguous_target_ids() {
        let mcp = TaskServer::new("http://example.com");
        let err = tool_error_json(
            mcp.follow_up(Parameters(FollowUpRequest {
                session_id: Some(Uuid::new_v4()),
                attempt_id: Some(Uuid::new_v4()),
                prompt: None,
                action: FollowUpAction::Cancel,
                variant: None,
                request_id: None,
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            err.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_AMBIGUOUS_TARGET)
        );
        assert_eq!(err.get("retryable").and_then(|v| v.as_bool()), Some(false));
        assert!(err
            .get("hint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("list_task_attempts"));
    }

    #[tokio::test]
    async fn follow_up_send_requires_prompt_at_runtime() {
        let mcp = TaskServer::new("http://example.com");
        let err = tool_error_json(
            mcp.follow_up(Parameters(FollowUpRequest {
                session_id: Some(Uuid::new_v4()),
                attempt_id: None,
                prompt: None,
                action: FollowUpAction::Send,
                variant: None,
                request_id: None,
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            err.get("code").and_then(|v| v.as_str()),
            Some("missing_required")
        );
        assert!(err.get("hint").and_then(|v| v.as_str()).unwrap_or("").contains("prompt"));
    }

    #[tokio::test]
    async fn closed_loop_tools_smoke() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();
        {
            let mut config = deployment.config().write().await;
            config.diff_preview_guard = DiffPreviewGuardPreset::Safe;
        }

        let repo_path = temp_root.join("repo");
        GitService::new()
            .initialize_repo_with_main_branch(&repo_path)
            .unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "MCP closed loop".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let repo = Repo::find_or_create(&deployment.db().pool, &repo_path, "Repo")
            .await
            .unwrap();
        ProjectRepo::create(&deployment.db().pool, project_id, repo.id)
            .await
            .unwrap();

        let task_title = "MCP closed loop task".to_string();
        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(project_id, task_title.clone(), None),
            task_id,
        )
        .await
        .unwrap();

        let branch_name = format!("mcp-closed-loop-{}", Uuid::new_v4());
        let attempt_id = Uuid::new_v4();
        let workspace = Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: branch_name.clone(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        WorkspaceRepo::create_many(
            &deployment.db().pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: "main".to_string(),
            }],
        )
        .await
        .unwrap();

        let workspace_dir_name =
            LocalContainerService::dir_name_from_workspace(&workspace.id, &task_title);
        let workspace_dir = WorkspaceManager::get_workspace_base_dir().join(&workspace_dir_name);
        WorkspaceManager::create_workspace(
            &workspace_dir,
            &[RepoWorkspaceInput::new(repo.clone(), "main".to_string())],
            &branch_name,
        )
        .await
        .unwrap();

        let worktree_path = workspace_dir.join(&repo.name);
        for i in 0..201 {
            std::fs::write(worktree_path.join(format!("file-{i}.txt")), "hi\n").unwrap();
        }

        let workspace_dir_str = workspace_dir.to_string_lossy().to_string();
        Workspace::update_container_ref(&deployment.db().pool, workspace.id, &workspace_dir_str)
            .await
            .unwrap();

        let (addr, backend_handle) = start_backend(deployment.clone()).await;
        let mcp = TaskServer::new(&format!("http://{addr}"));
        let attempt_id_str = attempt_id.to_string();

        // 1) Idle attempt status
        let status = tool_json(
            mcp.get_attempt_status(Parameters(GetAttemptStatusRequest { attempt_id }))
                .await
                .unwrap(),
        );
        assert_eq!(
            status.get("attempt_id").and_then(|v| v.as_str()),
            Some(attempt_id_str.as_str())
        );
        assert_eq!(status.get("state").and_then(|v| v.as_str()), Some("idle"));

        // 1b) follow_up by attempt_id before any session exists yields an actionable hint
        let no_session = tool_error_json(
            mcp.follow_up(Parameters(FollowUpRequest {
                session_id: None,
                attempt_id: Some(attempt_id),
                prompt: None,
                action: FollowUpAction::Cancel,
                variant: None,
                request_id: None,
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            no_session.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_NO_SESSION_YET)
        );
        assert_eq!(
            no_session.get("retryable").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(no_session
            .get("hint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("get_attempt_status"));

        // 2) No process yields empty logs
        let logs = tool_json(
            mcp.tail_attempt_logs(Parameters(TailAttemptLogsRequest {
                attempt_id,
                channel: None,
                limit: Some(2),
                cursor: None,
                after_entry_index: None,
            }))
            .await
            .unwrap(),
        );
        assert!(logs.get("execution_process_id").unwrap().is_null());
        assert_eq!(
            logs.pointer("/page/entries")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(0)
        );

        // 3) Changes guard blocks files unless forced
        let changes_blocked = tool_json(
            mcp.get_attempt_changes(Parameters(GetAttemptChangesRequest {
                attempt_id,
                force: Some(false),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            changes_blocked.get("blocked").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            changes_blocked.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_BLOCKED_GUARDRAILS)
        );
        assert!(changes_blocked
            .get("hint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("force=true"));
        assert_eq!(
            changes_blocked
                .get("blocked_reason")
                .and_then(|v| v.as_str()),
            Some("threshold_exceeded")
        );
        assert_eq!(
            changes_blocked
                .get("files")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(0)
        );

        let changes_forced = tool_json(
            mcp.get_attempt_changes(Parameters(GetAttemptChangesRequest {
                attempt_id,
                force: Some(true),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            changes_forced.get("blocked").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(
            changes_forced
                .get("files")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
                >= 201
        );

        // 3b) Patch is blocked by guardrails unless force=true
        let patch_blocked = tool_json(
            mcp.get_attempt_patch(Parameters(GetAttemptPatchRequest {
                attempt_id,
                paths: vec![format!("{}/file-0.txt", repo.name)],
                force: Some(false),
                max_bytes: Some(50_000),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            patch_blocked.get("blocked").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            patch_blocked.get("blocked_reason").and_then(|v| v.as_str()),
            Some("threshold_exceeded")
        );
        assert_eq!(
            patch_blocked.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_BLOCKED_GUARDRAILS)
        );
        assert!(patch_blocked
            .get("hint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("force=true"));

        let patch_forced = tool_json(
            mcp.get_attempt_patch(Parameters(GetAttemptPatchRequest {
                attempt_id,
                paths: vec![format!("{}/file-0.txt", repo.name)],
                force: Some(true),
                max_bytes: Some(50_000),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            patch_forced.get("blocked").and_then(|v| v.as_bool()),
            Some(false)
        );
        let patch_text = patch_forced.get("patch").and_then(|v| v.as_str()).unwrap_or("");
        assert!(patch_text.contains(&format!("+++ b/{}/file-0.txt", repo.name)));
        assert!(patch_text.contains("+hi"));

        // 4) Create a process + logs, then tail with cursor paging
        let session = Session::create(
            &deployment.db().pool,
            &CreateSession { executor: None },
            Uuid::new_v4(),
            workspace.id,
        )
        .await
        .unwrap();

        let action = ExecutorAction::new(
            ExecutorActionType::ScriptRequest(ScriptRequest {
                script: "true".to_string(),
                language: ScriptRequestLanguage::Bash,
                context: ScriptContext::SetupScript,
                working_dir: None,
            }),
            None,
        );

        let process_id = Uuid::new_v4();
        let process = ExecutionProcess::create(
            &deployment.db().pool,
            &CreateExecutionProcess {
                session_id: session.id,
                executor_action: action,
                run_reason: ExecutionProcessRunReason::CodingAgent,
            },
            process_id,
            &[],
        )
        .await
        .unwrap();

        for i in 1..=5i64 {
            let entry = PatchType::NormalizedEntry(NormalizedEntry {
                timestamp: None,
                entry_type: NormalizedEntryType::AssistantMessage,
                content: format!("hello {i}"),
                metadata: None,
            });
            let json = serde_json::to_string(&entry).unwrap();
            ExecutionProcessLogEntry::upsert_entry(
                &deployment.db().pool,
                process.id,
                LogEntryChannel::Normalized,
                i,
                &json,
            )
            .await
            .unwrap();
        }

        let status = tool_json(
            mcp.get_attempt_status(Parameters(GetAttemptStatusRequest { attempt_id }))
                .await
                .unwrap(),
        );
        assert_eq!(
            status.get("state").and_then(|v| v.as_str()),
            Some("running")
        );

        let process_id_str = process_id.to_string();
        let page1 = tool_json(
            mcp.tail_attempt_logs(Parameters(TailAttemptLogsRequest {
                attempt_id,
                channel: None,
                limit: Some(2),
                cursor: None,
                after_entry_index: None,
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            page1.get("execution_process_id").and_then(|v| v.as_str()),
            Some(process_id_str.as_str())
        );
        let next_cursor = page1
            .get("page")
            .and_then(|v| v.get("next_cursor"))
            .and_then(|v| v.as_i64())
            .expect("next_cursor");
        assert_eq!(next_cursor, 4);

        let page2 = tool_json(
            mcp.tail_attempt_logs(Parameters(TailAttemptLogsRequest {
                attempt_id,
                channel: None,
                limit: Some(2),
                cursor: Some(next_cursor),
                after_entry_index: None,
            }))
            .await
            .unwrap(),
        );
        let next_cursor2 = page2
            .get("page")
            .and_then(|v| v.get("next_cursor"))
            .and_then(|v| v.as_i64())
            .expect("next_cursor");
        assert_eq!(next_cursor2, 2);

        // 4b) Incremental tailing via after_entry_index (new entries only)
        let after_page = tool_json(
            mcp.tail_attempt_logs(Parameters(TailAttemptLogsRequest {
                attempt_id,
                channel: None,
                limit: Some(10),
                cursor: None,
                after_entry_index: Some(3),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            after_page
                .pointer("/page/entries/0/entry_index")
                .and_then(|v| v.as_i64()),
            Some(4)
        );
        assert_eq!(
            after_page
                .pointer("/page/entries/1/entry_index")
                .and_then(|v| v.as_i64()),
            Some(5)
        );

        // 4c) Mixed pagination modes are rejected with a hint
        let mixed_err = tool_error_json(
            mcp.tail_attempt_logs(Parameters(TailAttemptLogsRequest {
                attempt_id,
                channel: None,
                limit: Some(10),
                cursor: Some(4),
                after_entry_index: Some(3),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            mixed_err.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_MIXED_PAGINATION)
        );
        assert_eq!(
            mixed_err.get("retryable").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(mixed_err
            .get("hint")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("after_entry_index"));

        // 5) Stop a running attempt (force=true works even without an OS child process)
        let stop = tool_json(
            mcp.stop_attempt(Parameters(StopAttemptRequest {
                attempt_id,
                force: Some(true),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            stop.get("attempt_id").and_then(|v| v.as_str()),
            Some(attempt_id.to_string().as_str())
        );
        assert_eq!(stop.get("force").and_then(|v| v.as_bool()), Some(true));

        let status_after = tool_json(
            mcp.get_attempt_status(Parameters(GetAttemptStatusRequest { attempt_id }))
                .await
                .unwrap(),
        );
        assert_ne!(
            status_after.get("state").and_then(|v| v.as_str()),
            Some("running")
        );

        backend_handle.abort();
        WorkspaceManager::cleanup_workspace(&workspace_dir, &[repo])
            .await
            .unwrap();

        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn list_executors_returns_parseable_executor_ids() {
        let mcp = TaskServer::new("http://example.com");
        let result = tool_json(mcp.list_executors().await.unwrap());
        let executors = result
            .get("executors")
            .and_then(|v| v.as_array())
            .expect("executors array");
        assert!(!executors.is_empty());

        for item in executors {
            let executor = item
                .get("executor")
                .and_then(|v| v.as_str())
                .expect("executor string");
            let norm = executor.replace('-', "_").to_ascii_uppercase();
            assert!(BaseCodingAgent::from_str(&norm).is_ok(), "{executor}");

            assert!(item.get("supports_mcp").and_then(|v| v.as_bool()).is_some());
            assert!(item.get("default_variant").is_some());
        }
    }

    #[tokio::test]
    async fn tail_session_messages_pages_and_resolves_latest_session() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Session transcript".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(project_id, "Transcript task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "transcript-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let session_id = Uuid::new_v4();
        Session::create(
            &deployment.db().pool,
            &CreateSession { executor: None },
            session_id,
            attempt_id,
        )
        .await
        .unwrap();

        for i in 1..=3 {
            let action = ExecutorAction::new(
                ExecutorActionType::ScriptRequest(ScriptRequest {
                    script: "true".to_string(),
                    language: ScriptRequestLanguage::Bash,
                    context: ScriptContext::SetupScript,
                    working_dir: None,
                }),
                None,
            );
            let process_id = Uuid::new_v4();
            let process = ExecutionProcess::create(
                &deployment.db().pool,
                &CreateExecutionProcess {
                    session_id,
                    executor_action: action,
                    run_reason: ExecutionProcessRunReason::CodingAgent,
                },
                process_id,
                &[],
            )
            .await
            .unwrap();

            let turn_id = Uuid::new_v4();
            CodingAgentTurn::create(
                &deployment.db().pool,
                &CreateCodingAgentTurn {
                    execution_process_id: process.id,
                    prompt: Some(format!("prompt {i}")),
                },
                turn_id,
            )
            .await
            .unwrap();
            CodingAgentTurn::update_summary(
                &deployment.db().pool,
                process.id,
                &format!("summary {i}"),
            )
            .await
            .unwrap();
        }

        let (addr, backend_handle) = start_backend(deployment.clone()).await;
        let mcp = TaskServer::new(&format!("http://{addr}"));

        let page1 = tool_json(
            mcp.tail_session_messages(Parameters(TailSessionMessagesRequest {
                session_id: None,
                attempt_id: Some(attempt_id),
                limit: Some(2),
                cursor: None,
            }))
            .await
            .unwrap(),
        );

        assert_eq!(
            page1.get("session_id").and_then(|v| v.as_str()),
            Some(session_id.to_string().as_str())
        );
        assert_eq!(
            page1
                .pointer("/page/entries/0/prompt")
                .and_then(|v| v.as_str()),
            Some("prompt 2")
        );
        assert_eq!(
            page1
                .pointer("/page/entries/1/prompt")
                .and_then(|v| v.as_str()),
            Some("prompt 3")
        );

        let cursor = page1
            .pointer("/page/next_cursor")
            .and_then(|v| v.as_i64())
            .expect("next_cursor");

        let page2 = tool_json(
            mcp.tail_session_messages(Parameters(TailSessionMessagesRequest {
                session_id: Some(session_id),
                attempt_id: None,
                limit: Some(2),
                cursor: Some(cursor),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            page2
                .pointer("/page/entries/0/prompt")
                .and_then(|v| v.as_str()),
            Some("prompt 1")
        );
        assert!(page2.pointer("/page/entries/1").is_none());

        backend_handle.abort();
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn get_attempt_file_enforces_path_containment_and_size_limits() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "Attempt file".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(project_id, "File task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "file-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let workspace_dir = temp_root.join("workspace");
        std::fs::create_dir_all(workspace_dir.join("repo")).unwrap();
        std::fs::write(workspace_dir.join("repo/hello.txt"), "hello world").unwrap();
        let workspace_dir_str = workspace_dir.to_string_lossy().to_string();
        Workspace::update_container_ref(&deployment.db().pool, attempt_id, &workspace_dir_str)
            .await
            .unwrap();

        let (addr, backend_handle) = start_backend(deployment.clone()).await;
        let mcp = TaskServer::new(&format!("http://{addr}"));

        let ok = tool_json(
            mcp.get_attempt_file(Parameters(GetAttemptFileRequest {
                attempt_id,
                path: "repo/hello.txt".to_string(),
                start: None,
                max_bytes: Some(5),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(ok.get("blocked").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(ok.get("truncated").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(ok.get("bytes").and_then(|v| v.as_u64()), Some(5));
        assert_eq!(
            ok.get("content").and_then(|v| v.as_str()),
            Some("hello")
        );

        let outside = tool_json(
            mcp.get_attempt_file(Parameters(GetAttemptFileRequest {
                attempt_id,
                path: "../outside.txt".to_string(),
                start: None,
                max_bytes: Some(10),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            outside.get("blocked").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            outside.get("blocked_reason").and_then(|v| v.as_str()),
            Some("path_outside_workspace")
        );
        assert_eq!(
            outside.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_BLOCKED_GUARDRAILS)
        );

        let too_big = tool_json(
            mcp.get_attempt_file(Parameters(GetAttemptFileRequest {
                attempt_id,
                path: "repo/hello.txt".to_string(),
                start: None,
                max_bytes: Some(600 * 1024),
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            too_big.get("blocked").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            too_big.get("blocked_reason").and_then(|v| v.as_str()),
            Some("size_exceeded")
        );

        backend_handle.abort();
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[tokio::test]
    async fn tail_session_messages_no_session_yet_hint_mentions_retry_tool() {
        let temp_root = std::env::temp_dir().join(format!("vk-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).unwrap();

        let db_path = temp_root.join("db.sqlite");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let env_guard = TestEnvGuard::new(&temp_root, db_url);

        let deployment = DeploymentImpl::new().await.unwrap();

        let project_id = Uuid::new_v4();
        Project::create(
            &deployment.db().pool,
            &CreateProject {
                name: "No session yet".to_string(),
                repositories: Vec::new(),
            },
            project_id,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        Task::create(
            &deployment.db().pool,
            &CreateTask::from_title_description(project_id, "No session task".to_string(), None),
            task_id,
        )
        .await
        .unwrap();

        let attempt_id = Uuid::new_v4();
        Workspace::create(
            &deployment.db().pool,
            &CreateWorkspace {
                branch: "no-session-branch".to_string(),
                agent_working_dir: None,
            },
            attempt_id,
            task_id,
        )
        .await
        .unwrap();

        let (addr, backend_handle) = start_backend(deployment.clone()).await;
        let mcp = TaskServer::new(&format!("http://{addr}"));

        let err = tool_error_json(
            mcp.tail_session_messages(Parameters(TailSessionMessagesRequest {
                session_id: None,
                attempt_id: Some(attempt_id),
                limit: Some(10),
                cursor: None,
            }))
            .await
            .unwrap(),
        );
        assert_eq!(
            err.get("code").and_then(|v| v.as_str()),
            Some(MCP_CODE_NO_SESSION_YET)
        );
        let hint = err.get("hint").and_then(|v| v.as_str()).unwrap_or("");
        assert!(hint.contains("get_attempt_status"));
        assert!(hint.contains("tail_session_messages"));

        backend_handle.abort();
        drop(env_guard);
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
