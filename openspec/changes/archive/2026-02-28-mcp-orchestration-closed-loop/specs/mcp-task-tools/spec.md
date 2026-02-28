## MODIFIED Requirements

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

## ADDED Requirements

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
