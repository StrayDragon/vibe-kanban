# mcp-task-tools Specification

## Purpose
TBD - created by archiving change redesign-mcp-task-tools. Update Purpose after archive.
## Requirements
### Requirement: MCP task tool set
The system SHALL expose a coherent MCP tool set for task operations with consistent naming:
- list_projects
- list_repos
- list_tasks
- list_task_attempts
- get_task
- create_task
- update_task
- delete_task
- start_task_attempt
- follow_up
- get_attempt_status
- tail_attempt_logs
- get_attempt_changes
- get_context (only when workspace context is available)

#### Scenario: Tools are discoverable
- **WHEN** a client queries the MCP server for available tools
- **THEN** the tool list includes list_tasks and list_task_attempts with the names above

### Requirement: Attempt terminology and identifiers
The system SHALL use the term "attempt" in MCP schemas for task execution workspaces. The system SHALL expose `attempt_id` as the canonical identifier and map it internally to the workspace ID. All `*_id` fields SHALL be UUID strings and all timestamp fields SHALL be RFC3339 strings.

#### Scenario: Attempt id is usable for follow-up
- **WHEN** a client receives an attempt_id from list_task_attempts
- **THEN** that attempt_id can be used in follow_up without additional ID translation

#### Scenario: No workspace_id alias in MCP schemas
- **WHEN** a client inspects MCP responses for task attempts
- **THEN** identifiers are exposed as attempt_id and workspace_id is not present

### Requirement: list_tasks provides attempt summary
The list_tasks tool SHALL accept a project_id and optional status/limit filters. Latest attempt selection SHALL use the most recently created workspace (ORDER BY workspace.created_at DESC, attempt_id ASC). Each task in the response SHALL include an attempt summary containing:
- latest_attempt_id (nullable)
- latest_workspace_branch (nullable)
- latest_session_id (nullable)
- latest_session_executor (nullable)
- has_in_progress_attempt (boolean)
- last_attempt_failed (boolean)

#### Scenario: Task list includes latest attempt info
- **WHEN** a client calls list_tasks for a project with existing attempts
- **THEN** each task includes the latest attempt and session identifiers based on workspace.created_at ordering

### Requirement: list_task_attempts returns attempt details
The list_task_attempts tool SHALL accept a task_id and return attempts ordered by workspace.created_at DESC with attempt_id ASC tie-break. Attempts SHALL include:
- attempt_id
- workspace_branch
- created_at
- updated_at
- latest_session_id (nullable)
- latest_session_executor (nullable)
The response SHALL also include top-level latest_attempt_id and latest_session_id summary fields derived from the most recently created workspace.

#### Scenario: Attempts returned with session summary
- **WHEN** a client calls list_task_attempts for a task
- **THEN** the response includes attempt entries and latest summary fields

#### Scenario: Attempt has no sessions
- **WHEN** a task attempt has no session yet
- **THEN** latest_session_id is null for that attempt and for the top-level summary

### Requirement: follow_up targeting
The follow_up tool SHALL accept either attempt_id or session_id. When attempt_id is provided, the tool SHALL resolve the latest session for that attempt and execute the action (send/queue/cancel).

#### Scenario: Follow-up by attempt id
- **WHEN** a client calls follow_up with attempt_id and action=send
- **THEN** the server resolves the latest session for that attempt and triggers a follow-up execution

### Requirement: Attempt status tool
The system SHALL expose a `get_attempt_status` MCP tool that accepts `attempt_id` (UUID string) and returns attempt/workspace metadata plus the latest session and execution process summary for that attempt.

The response SHALL include:
- attempt_id (UUID string)
- task_id (UUID string)
- workspace_branch (string)
- created_at, updated_at (RFC3339 strings)
- latest_session_id (nullable UUID string)
- latest_execution_process_id (nullable UUID string)
- state (`idle | running | completed | failed`)
- last_activity_at (nullable RFC3339 string)
- failure_summary (nullable string)

#### Scenario: Attempt has no sessions or processes
- **WHEN** a client calls get_attempt_status for an attempt with no sessions and no execution processes
- **THEN** state is `idle` and latest_session_id and latest_execution_process_id are null

#### Scenario: Attempt is running
- **WHEN** a client calls get_attempt_status for an attempt with a non-dev-server execution process in `running`
- **THEN** state is `running` and latest_execution_process_id is set

#### Scenario: Attempt is failed
- **WHEN** a client calls get_attempt_status for an attempt whose latest non-dev-server execution process is `failed` or `killed`
- **THEN** state is `failed` and failure_summary is non-empty

### Requirement: Attempt log tail tool
The system SHALL expose a `tail_attempt_logs` MCP tool that accepts:
- attempt_id (UUID string)
- channel (`normalized | raw`, default `normalized`)
- limit (optional integer; server applies a safe cap)
- cursor (optional integer; used to page older entries)

The tool SHALL return a tail-first history page in chronological order and SHALL preserve the existing cursor semantics of the execution-process log history v2 APIs.

#### Scenario: Default tail load
- **WHEN** a client calls tail_attempt_logs without cursor
- **THEN** the response contains the most recent N entries for the attempt in chronological order and indicates whether older history exists

#### Scenario: Load older history
- **WHEN** a client calls tail_attempt_logs with cursor returned by a prior call
- **THEN** the response returns the next older page and returns a new cursor when more history is available

#### Scenario: No process yields empty history
- **WHEN** a client calls tail_attempt_logs for an attempt with no relevant execution process
- **THEN** the response returns an empty entries list and has_more is false

### Requirement: Attempt changes snapshot tool
The system SHALL expose a `get_attempt_changes` MCP tool that accepts:
- attempt_id (UUID string)
- force (optional boolean, default false)

The tool SHALL return a diff summary and a changed-file list for the attempt without streaming and without returning full file contents.

The response SHALL include:
- summary (file_count, added, deleted, total_bytes)
- blocked (boolean)
- blocked_reason (nullable string; `summary_failed | threshold_exceeded`)
- files (list of changed files; empty when blocked)

Changed file paths SHALL be stable and unambiguous for multi-repo attempts (e.g., prefixed by repo name).

#### Scenario: Guard blocks file list
- **WHEN** the diff summary exceeds the active guard thresholds and force is false
- **THEN** blocked is true and files is empty while summary is still returned

#### Scenario: Force bypasses guard
- **WHEN** the diff summary exceeds the active guard thresholds and force is true
- **THEN** blocked is false and the response includes a changed-file list

### Requirement: Schema field documentation
Every MCP tool request/response field SHALL include a schema description that explains meaning, format, and allowable values (UUID, RFC3339, enum values).

#### Scenario: Field docs are present
- **WHEN** a client inspects the MCP schema for list_task_attempts
- **THEN** every field has a non-empty description clarifying its usage

### Requirement: Workspace context data
The get_context tool SHALL return the active project, task, and attempt metadata when available, including:
- project_id, project_name
- task_id, task_title, task_status
- attempt_id, workspace_branch
- workspace_repos with repo_id, repo_name, and target_branch

#### Scenario: Context returns unified attempt data
- **WHEN** get_context is available and called
- **THEN** the response includes attempt_id and workspace_branch alongside project/task data

### Requirement: MCP follow-up tool
The system SHALL expose an MCP tool that manages follow-up actions for a session. The tool SHALL accept either session_id or attempt_id and an action of send, queue, or cancel. The tool SHALL require a prompt for send and queue actions and MAY accept an optional variant.

#### Scenario: Send follow-up by attempt id
- **WHEN** a client calls the tool with attempt_id, action=send, and a prompt
- **THEN** the server resolves the latest session for the attempt and triggers a follow-up execution

#### Scenario: Send follow-up by session id
- **WHEN** a client calls the tool with session_id, action=send, and a prompt
- **THEN** the server triggers a follow-up execution for that session

#### Scenario: Queue follow-up by attempt id
- **WHEN** a client calls the tool with attempt_id, action=queue, and a prompt
- **THEN** the server queues the follow-up message for the latest session and returns queue status

#### Scenario: Queue follow-up by session id
- **WHEN** a client calls the tool with session_id, action=queue, and a prompt
- **THEN** the server queues the follow-up message for that session and returns queue status

#### Scenario: Cancel queued follow-up
- **WHEN** a client calls the tool with action=cancel for a session
- **THEN** the server cancels the queued follow-up message and returns queue status

#### Scenario: Attempt has no sessions
- **WHEN** a client calls the tool with attempt_id and no session exists
- **THEN** the tool returns an explicit error

### Requirement: Agent-guided tool descriptions
The MCP server SHALL provide tool descriptions that guide an LLM agent to the correct call and parameters. Tool descriptions MUST use a compact, consistent template that includes:

- `Use when:` (primary intent)
- `Required:` (required fields for the common case)
- `Optional:` (commonly used optional fields)
- `Next:` (the recommended next tool call in the workflow)
- `Avoid:` (common mistake(s) that lead to mis-calls)

#### Scenario: Tool description contains guidance headings
- **WHEN** a client inspects the MCP tool description for `follow_up`
- **THEN** the description includes the headings `Use when:`, `Required:`, `Next:`, and `Avoid:`

### Requirement: Mode-specific schemas for mode-specific requirements
For any MCP tool where required fields vary by mode (e.g. action-specific behavior), the request schema MUST enforce the mode-specific required fields (e.g. via `oneOf` / tagged enums) instead of relying only on runtime validation.

#### Scenario: Follow-up send requires prompt by schema
- **WHEN** a client inspects the MCP schema for `follow_up`
- **THEN** the schema indicates that `prompt` is required when `action=send`

### Requirement: Mutual exclusion for ambiguous targeting
When an MCP tool can target either `attempt_id` or `session_id`, the request MUST be unambiguous: clients MUST provide exactly one target identifier, and the server SHALL reject requests that provide none or both.

#### Scenario: Reject dual targeting
- **WHEN** a client calls `follow_up` with both `attempt_id` and `session_id`
- **THEN** the server returns an invalid-argument error with a hint to supply exactly one target identifier

### Requirement: Normalized pagination semantics for tailing and history
MCP tools that return ordered history MUST distinguish “older paging” from “incremental tailing”:

- `cursor` means “page older history”
- `after_*` means “return only items newer than X”
- the server MUST reject requests that supply both `cursor` and `after_*`

#### Scenario: Reject mixed pagination modes
- **WHEN** a client calls `tail_attempt_logs` with both `cursor` and `after_entry_index`
- **THEN** the server returns an invalid-argument error with a hint to use only one pagination mode

### Requirement: Executor discovery tool
The system SHALL expose a `list_executors` MCP tool that returns the available executor identifiers and their variants, plus basic capability flags sufficient for an agent to choose a valid executor without hard-coding strings.

The response MUST include, per executor:
- `executor` (stable identifier)
- `variants` (possibly empty)
- `supports_mcp` (boolean)
- `default_variant` (nullable)

#### Scenario: Executor identifiers are usable
- **WHEN** a client selects an `executor` returned by `list_executors`
- **THEN** the value can be used in `start_task_attempt.executor` without further translation

### Requirement: Stop attempt tool
The system SHALL expose a `stop_attempt` MCP tool that stops a running attempt’s relevant execution process (excluding dev servers). The tool SHALL accept:

- `attempt_id`
- `force` (optional boolean; when true, perform a hard stop)

#### Scenario: Stop a running attempt
- **WHEN** an attempt is `running` and a client calls `stop_attempt`
- **THEN** the attempt transitions to `failed` or `completed` and `get_attempt_status.state` is no longer `running`

### Requirement: Session transcript tail tool
The system SHALL expose a `tail_session_messages` MCP tool that provides a bounded, paginated replay of the session transcript suitable for LLM context restoration.

The tool SHALL accept either `session_id` or `attempt_id` (resolving to the latest session), plus `cursor`/`limit` for older paging.

#### Scenario: Tail transcript for latest session in attempt
- **WHEN** a client calls `tail_session_messages` with `attempt_id`
- **THEN** the server resolves the latest session and returns the most recent transcript entries with a cursor for older entries

### Requirement: Bounded attempt artifact retrieval tools
The system SHALL expose MCP tools for bounded artifact retrieval from an attempt workspace:

- `get_attempt_file` (read a file range or bounded byte slice)
- `get_attempt_patch` (retrieve a patch for selected paths)

Both tools MUST enforce size limits and MUST return explicit blocking signals:
- `blocked` (boolean)
- `blocked_reason` (e.g. `path_outside_workspace | size_exceeded | too_many_paths`)
- `truncated` (boolean; when partial data is returned)

#### Scenario: Artifact retrieval is size-bounded
- **WHEN** a client calls `get_attempt_file` with a request exceeding configured size limits
- **THEN** the response sets `blocked=true` with a `blocked_reason` and a hint to narrow the request

### Requirement: Actionable MCP error envelope
When an MCP tool call fails due to an expected recoverable condition, the tool MUST return a structured error payload that includes:

- `code` (stable string)
- `retryable` (boolean)
- `hint` (actionable next step, preferably naming a tool and required field)
- `details` (optional small JSON object)

#### Scenario: Missing session yields actionable hint
- **WHEN** a client calls `follow_up` by `attempt_id` before any session exists
- **THEN** the error payload includes a `code` indicating no session exists and a `hint` to call `get_attempt_status` and retry once `latest_session_id` is non-null

### Requirement: request_id idempotency for mutating MCP tools
The MCP server SHALL support safe retries for mutating tool calls by accepting an optional `request_id` (idempotency key) on mutating MCP tools that create resources or spawn execution, notably:

- `create_task`
- `start_task_attempt`
- `follow_up` when `action=send` or `action=queue`

When `request_id` is provided, the server MUST treat repeated calls with the same `request_id` and the same effective request payload as idempotent and MUST return the same result.

If a client reuses the same `request_id` with a different effective request payload, the server MUST reject the request with a conflict error and an actionable hint.

#### Scenario: create_task retry returns same task id
- **WHEN** a client calls `create_task` twice with the same `request_id` and same payload
- **THEN** both responses contain the same `task_id`

#### Scenario: start_task_attempt retry returns same attempt id
- **WHEN** a client calls `start_task_attempt` twice with the same `request_id` and same payload
- **THEN** both responses contain the same `attempt_id`

#### Scenario: request_id reuse with different payload is rejected
- **WHEN** a client calls `create_task` with a `request_id` that was previously used for a different `create_task` payload
- **THEN** the server responds with a conflict error and a hint to generate a new `request_id`

