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
The system SHALL expose an MCP tool that manages follow-up actions for a session. The tool SHALL accept either session_id or workspace_id and an action of send, queue, or cancel. The tool SHALL require a prompt for send and queue actions and MAY accept an optional variant.

#### Scenario: Send follow-up by workspace id
- **WHEN** a client calls the tool with workspace_id, action=send, and a prompt
- **THEN** the server resolves the latest session for the workspace and triggers a follow-up execution

#### Scenario: Send follow-up by session id
- **WHEN** a client calls the tool with session_id, action=send, and a prompt
- **THEN** the server triggers a follow-up execution for that session

#### Scenario: Queue follow-up by workspace id
- **WHEN** a client calls the tool with workspace_id, action=queue, and a prompt
- **THEN** the server queues the follow-up message for the latest session and returns queue status

#### Scenario: Queue follow-up by session id
- **WHEN** a client calls the tool with session_id, action=queue, and a prompt
- **THEN** the server queues the follow-up message for that session and returns queue status

#### Scenario: Cancel queued follow-up
- **WHEN** a client calls the tool with action=cancel for a session
- **THEN** the server cancels the queued follow-up message and returns queue status

#### Scenario: Workspace has no sessions
- **WHEN** a client calls the tool with workspace_id and no session exists
- **THEN** the tool returns an explicit error

