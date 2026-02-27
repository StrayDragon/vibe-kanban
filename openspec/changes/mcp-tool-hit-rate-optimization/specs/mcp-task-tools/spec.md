## ADDED Requirements

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
