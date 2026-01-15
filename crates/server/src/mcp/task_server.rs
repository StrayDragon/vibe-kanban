use std::{cmp::Ordering, str::FromStr};

use db::models::{
    execution_process::ExecutionProcess,
    project::Project,
    repo::Repo,
    session::Session,
    tag::Tag,
    task::{CreateTask, Task, TaskStatus, TaskWithAttemptStatus, UpdateTask},
    workspace::{Workspace, WorkspaceContext},
};
use executors::{executors::{BaseCodingAgent, CodingAgent}, profile::ExecutorProfileId};
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

use crate::mcp::params::VkParameters;
use crate::routes::{
    containers::ContainerQuery,
    task_attempts::{CreateTaskAttemptBody, WorkspaceRepoInput},
};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateTaskRequest {
    #[schemars(description = "The ID of the project to create the task in (UUID string). This is required!")]
    pub project_id: Uuid,
    #[schemars(description = "The title of the task")]
    pub title: String,
    #[schemars(description = "Optional description of the task")]
    pub description: Option<String>,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FollowUpRequest {
    #[schemars(description = "The session ID to target for the follow-up action (UUID string)")]
    pub session_id: Option<Uuid>,
    #[schemars(
        description = "The attempt ID whose latest session should be used (UUID string)"
    )]
    pub attempt_id: Option<Uuid>,
    #[schemars(description = "The follow-up prompt for send/queue actions")]
    pub prompt: Option<String>,
    #[schemars(description = "The follow-up action to perform")]
    pub action: FollowUpAction,
    #[schemars(description = "Optional executor variant for this follow-up")]
    pub variant: Option<String>,
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
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => Some(
                "Check tool inputs and IDs. Use list_* tools to fetch valid UUIDs.",
            ),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Some(
                "Backend rejected the request. Ensure the backend is running locally and access is allowed.",
            ),
            StatusCode::NOT_FOUND => {
                Some("Resource not found. Verify IDs via list_* tools or get_context.")
            }
            StatusCode::CONFLICT => Some(
                "Conflict detected. Check for existing resources or adjust input values.",
            ),
            StatusCode::TOO_MANY_REQUESTS => {
                Some("Rate limited. Wait a moment and retry the request.")
            }
            StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT => Some(
                "Backend is unhealthy or unavailable. Start/restart the backend and retry.",
            ),
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
            return Err(
                Self::err_with(
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
                .unwrap(),
            );
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
    ) -> Result<Uuid, CallToolResult> {
        if let Some(session_id) = session_id {
            return Ok(session_id);
        }

        let attempt_id = match attempt_id {
            Some(attempt_id) => attempt_id,
            None => {
                return Err(
                    Self::err_with(
                        "session_id or attempt_id is required",
                        None,
                        Some(
                            "Provide attempt_id from list_task_attempts or session_id from get_context."
                                .to_string(),
                        ),
                        Some("missing_required"),
                        None,
                    )
                    .unwrap(),
                );
            }
        };

        let url = self.url(&format!("/api/sessions?workspace_id={}", attempt_id));
        let sessions: Vec<Session> =
            match self.send_json(self.client.get(&url), "GET", &url).await {
                Ok(sessions) => sessions,
                Err(e) => return Err(e),
            };

        let latest = sessions.into_iter().max_by_key(|session| session.created_at);
        let Some(latest) = latest else {
            return Err(
                Self::err_with(
                    "No sessions found for attempt",
                    Some(serde_json::json!({ "attempt_id": attempt_id.to_string() })),
                    Some("Call list_task_attempts to confirm attempts or use get_context."
                        .to_string()),
                    Some("not_found"),
                    None,
                )
                .unwrap(),
            );
        };

        Ok(latest.id)
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
        let summaries: Vec<TaskAttemptSummaryEntry> =
            match self.send_json(self.client.post(&url).json(&payload), "POST", &url).await {
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
            .min_by(|a, b| Self::compare_attempts_newest_first(*a, *b));

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
        description = "Return project, task, and attempt metadata for the current workspace session context."
    )]
    async fn get_context(&self) -> Result<CallToolResult, ErrorData> {
        // Context was fetched at startup and cached
        // This tool is only registered if context exists, so unwrap is safe
        let context = self.context.as_ref().expect("VK context should exist");
        TaskServer::success(context)
    }

    #[tool(
        description = "Create a new task/ticket in a project. Always pass the `project_id` of the project you want to create the task in - it is required!"
    )]
    async fn create_task(
        &self,
        VkParameters(CreateTaskRequest {
            project_id,
            title,
            description,
        }): VkParameters<CreateTaskRequest>,
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

        let task: Task = match self
            .send_json(
                self.client
                    .post(&url)
                    .json(&CreateTask::from_title_description(
                        project_id,
                        title,
                        expanded_description,
                    )),
                "POST",
                &url,
            )
            .await
        {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        TaskServer::success(&CreateTaskResponse {
            task_id: task.id.to_string(),
        })
    }

    #[tool(description = "List all the available projects")]
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

    #[tool(description = "List all repositories for a project. `project_id` is required!")]
    async fn list_repos(
        &self,
        VkParameters(ListReposRequest { project_id }): VkParameters<ListReposRequest>,
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
        description = "List all the task/tickets in a project with optional filtering and execution status. `project_id` is required!"
    )]
    async fn list_tasks(
        &self,
        VkParameters(ListTasksRequest {
            project_id,
            status,
            limit,
        }): VkParameters<ListTasksRequest>,
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
        description = "List all attempts for a task, including latest session details. `task_id` is required!"
    )]
    async fn list_task_attempts(
        &self,
        VkParameters(ListTaskAttemptsRequest { task_id }): VkParameters<ListTaskAttemptsRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut attempts = match self.fetch_attempts_with_latest_session(task_id).await {
            Ok(attempts) => attempts,
            Err(e) => return Ok(e),
        };

        attempts.sort_by(Self::compare_attempts_newest_first);

        let attempt_details: Vec<TaskAttemptDetails> = attempts
            .iter()
            .map(Self::attempt_details_from)
            .collect();
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
        description = "Start working on a task by creating and launching a new attempt (workspace)."
    )]
    async fn start_task_attempt(
        &self,
        VkParameters(StartTaskAttemptRequest {
            task_id,
            executor,
            variant,
            repos,
        }): VkParameters<StartTaskAttemptRequest>,
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
        let workspace: Workspace =
            match self.send_json(self.client.post(&url).json(&payload), "POST", &url).await
        {
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
        description = "Manage follow-up actions for a session. Provide `session_id` or `attempt_id`, plus action=send|queue|cancel."
    )]
    async fn follow_up(
        &self,
        VkParameters(FollowUpRequest {
            session_id,
            attempt_id,
            prompt,
            action,
            variant,
        }): VkParameters<FollowUpRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let session_id = match self.resolve_session_id(session_id, attempt_id).await {
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
                let execution_process: ExecutionProcess =
                    match self.send_json(self.client.post(&url).json(&payload), "POST", &url).await
                    {
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
                let status: QueueStatus =
                    match self.send_json(self.client.post(&url).json(&payload), "POST", &url).await
                    {
                        Ok(status) => status,
                        Err(e) => return Ok(e),
                    };

                TaskServer::success(&FollowUpResponse::from_status(session_id, action, status))
            }
            FollowUpAction::Cancel => {
                let url = self.url(&format!("/api/sessions/{}/queue", session_id));
                let status: QueueStatus =
                    match self.send_json(self.client.delete(&url), "DELETE", &url).await {
                        Ok(status) => status,
                        Err(e) => return Ok(e),
                    };

                TaskServer::success(&FollowUpResponse::from_status(session_id, action, status))
            }
        }
    }

    #[tool(
        description = "Update an existing task/ticket's title, description, or status. `task_id` is required; `title`, `description`, and `status` are optional."
    )]
    async fn update_task(
        &self,
        VkParameters(UpdateTaskRequest {
            task_id,
            title,
            description,
            status,
        }): VkParameters<UpdateTaskRequest>,
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
        let updated_task: Task =
            match self.send_json(self.client.put(&url).json(&payload), "PUT", &url).await
        {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let details = TaskDetails::from_task(updated_task);
        let repsonse = UpdateTaskResponse { task: details };
        TaskServer::success(&repsonse)
    }

    #[tool(
        description = "Delete a task/ticket. `task_id` is required!"
    )]
    async fn delete_task(
        &self,
        VkParameters(DeleteTaskRequest { task_id }): VkParameters<DeleteTaskRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let url = self.url(&format!("/api/tasks/{}", task_id));
        if let Err(e) = self
            .send_json::<serde_json::Value>(self.client.delete(&url), "DELETE", &url)
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
        description = "Get detailed information (like task description) about a specific task/ticket. Use `list_tasks` to find task IDs. `task_id` is required!"
    )]
    async fn get_task(
        &self,
        VkParameters(GetTaskRequest { task_id }): VkParameters<GetTaskRequest>,
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
        let mut instruction = "A task and project management server. Use list tools to discover ids, then perform create/update/start/follow-up actions. TOOLS: 'list_projects', 'list_repos', 'list_tasks', 'list_task_attempts', 'get_task', 'create_task', 'update_task', 'delete_task', 'start_task_attempt', 'follow_up'. Use `list_tasks` to get task ids and latest attempt/session summaries. Use `list_task_attempts` to inspect attempts and get attempt_id/session_id for follow-up. The `follow_up` tool accepts either attempt_id (preferred) or session_id. attempt_id refers to the task attempt workspace id. Tool errors are returned as JSON with fields like error, hint, code, and retryable; invalid parameter errors include JSON-RPC error data with path/hint.".to_string();
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
