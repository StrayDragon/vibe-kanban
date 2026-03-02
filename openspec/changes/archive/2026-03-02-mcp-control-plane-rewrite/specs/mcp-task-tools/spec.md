# mcp-task-tools Specification

## Purpose
将 Vibe Kanban 的 MCP 接口从“HTTP 代理”重写为本地优先的 control plane，并围绕 project/task/attempt + approvals + feed 提供稳定、可恢复、可高频调用的工具集。

## MODIFIED Requirements

### Requirement: MCP task tool set
The system SHALL expose a coherent MCP tool set for task, attempt, approvals, and activity operations with consistent naming and schemas.

The tool set SHALL include:
- Discovery: `list_projects`, `list_repos`, `list_executors`, `cli_dependency_preflight`
- Tasks: `list_tasks`, `get_task`, `create_task`, `update_task`, `delete_task`
- Attempts: `list_task_attempts`, `start_attempt`, `send_follow_up`, `stop_attempt`
- Observation: `tail_project_activity`, `tail_task_activity`, `tail_attempt_feed`, `tail_session_messages`
- Changes/artifacts: `get_attempt_changes`, `get_attempt_patch`, `get_attempt_file`
- Approvals: `list_approvals`, `get_approval`, `respond_approval`

#### Scenario: Tools are discoverable
- **WHEN** a client queries the MCP server for available tools
- **THEN** the tool list includes `tail_attempt_feed` and `respond_approval`

### Requirement: Start attempt is single-roundtrip for initial prompt
The system SHALL provide `start_attempt` that creates an attempt and ensures a session exists. If `prompt` is provided, the server SHALL enqueue/send it without requiring a separate follow-up call.

#### Scenario: Start attempt with prompt
- **WHEN** a client calls `start_attempt` with `prompt`
- **THEN** the response includes both `attempt_id` and `session_id` and the attempt begins producing logs

### Requirement: Consistent pagination semantics
MCP tools that return ordered history MUST distinguish “older paging” from “incremental tailing”:
- `cursor` means “page older history”
- `after_*` means “return only items newer than X”
- the server MUST reject requests that supply both `cursor` and `after_*`

#### Scenario: Reject mixed pagination modes
- **WHEN** a client calls any `tail_*` tool with both `cursor` and `after_*`
- **THEN** the server returns an invalid-argument error with a hint to use only one mode

