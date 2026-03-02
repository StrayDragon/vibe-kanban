# mcp-task-tools Specification

## Purpose
将 Vibe Kanban 的 MCP 接口作为外部编排器（OpenClaw 类）的 control plane，并围绕 project/task/attempt + approvals + feed 提供稳定、可恢复、可高频调用的工具集。

## MODIFIED Requirements

### Requirement: MCP task tool set
The system SHALL expose a coherent MCP tool set for task, attempt, approvals, and activity operations with consistent naming and schemas.

The tool set SHALL include:
- Discovery: `list_projects`, `list_repos`, `list_executors`, `cli_dependency_preflight`
- Tasks: `list_tasks`, `get_task`, `create_task`, `update_task`, `delete_task`
- Attempts: `list_task_attempts`, `start_attempt`, `send_follow_up`, `stop_attempt`
- Attempt control (lease): `claim_attempt_control`, `get_attempt_control`, `release_attempt_control`
- Observation: `tail_project_activity`, `tail_task_activity`, `tail_attempt_feed`, `tail_session_messages`
- Changes/artifacts: `get_attempt_changes`, `get_attempt_patch`, `get_attempt_file`
- Approvals: `list_approvals`, `get_approval`, `respond_approval`

#### Scenario: Tools are discoverable
- **WHEN** a client queries the MCP server for available tools
- **THEN** the tool list includes `tail_attempt_feed`, `respond_approval`, and `claim_attempt_control`

### Requirement: Start attempt is single-roundtrip for initial prompt
The system SHALL provide `start_attempt` that creates an attempt and ensures a session exists. If `prompt` is provided, the server SHALL enqueue/send it without requiring a separate follow-up call.

`start_attempt` SHALL return a `control_token` representing a lease for mutating attempt operations.

#### Scenario: Start attempt with prompt
- **WHEN** a client calls `start_attempt` with `prompt`
- **THEN** the response includes `attempt_id`, `session_id`, and `control_token` and the attempt begins producing logs

## ADDED Requirements

### Requirement: Mutating attempt tools SHALL require a valid control_token
The system SHALL require a valid `control_token` for mutating attempt operations such as `send_follow_up` and `stop_attempt`.

#### Scenario: Follow-up requires control token
- **WHEN** a client calls `send_follow_up` without a valid `control_token`
- **THEN** the server returns a structured tool error (`isError=true`) with `code=invalid_control_token` or `code=attempt_claim_required`

### Requirement: Attempt control lease can be claimed and released
The system SHALL expose tools to claim, read, and release attempt control. A claim SHALL have a TTL and SHALL be reclaimable after expiry.

#### Scenario: Claim conflicts when lease is held
- **WHEN** a client calls `claim_attempt_control` for an attempt with an unexpired lease held by another client
- **THEN** the server returns a structured tool error (`isError=true`) with `code=attempt_claim_conflict` and a hint describing the current owner and expiry

#### Scenario: Lease can be released
- **WHEN** a client calls `release_attempt_control` with the matching `control_token`
- **THEN** the attempt becomes claimable by another client

