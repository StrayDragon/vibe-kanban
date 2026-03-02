# mcp-approvals Specification

## Purpose
为外部编排器（OpenClaw 类）提供纯 MCP 的 approvals 闭环：可列出/读取/响应 tool approvals，并保证在后端重启后仍可恢复处理，避免 attempt 卡死。

## ADDED Requirements

### Requirement: Approvals are persisted and attempt-scoped
The system SHALL persist tool approvals in durable storage. Each approval SHALL be associated with an `attempt_id` and an `execution_process_id`.

#### Scenario: Approval survives restart
- **WHEN** an approval is pending and the backend restarts
- **THEN** the approval remains listable and respondable via MCP

### Requirement: MCP can list approvals
The system SHALL expose an MCP tool that lists approvals filtered by `attempt_id` (and optionally `status`).

#### Scenario: List pending approvals for an attempt
- **WHEN** a client calls `list_approvals` with `attempt_id` and `status=pending`
- **THEN** the response includes every pending approval for that attempt

### Requirement: MCP can fetch approval details
The system SHALL expose an MCP tool to fetch an approval by `approval_id`, including `tool_name`, `tool_input`, and timestamps needed to render an approval prompt.

#### Scenario: Fetch approval details
- **WHEN** a client calls `get_approval` with a valid `approval_id`
- **THEN** the response includes `tool_name`, `tool_input`, `tool_call_id`, `created_at`, and `timeout_at`

### Requirement: MCP can respond to approvals with idempotency
The system SHALL expose an MCP tool to respond to an approval with `approved` or `denied` (and optional denial `reason`). The tool SHALL support `request_id` for safe retries.

#### Scenario: Approve a pending approval
- **WHEN** a client calls `respond_approval` for a pending approval with `approved=true`
- **THEN** the approval transitions to `approved` and the waiting executor is unblocked

#### Scenario: Deny with a reason
- **WHEN** a client calls `respond_approval` with `approved=false` and a `reason`
- **THEN** the approval transitions to `denied` and the reason is recorded

#### Scenario: Respond is idempotent
- **WHEN** a client retries the same `respond_approval` call with the same `request_id`
- **THEN** the server returns the previously recorded result without applying a duplicate transition

