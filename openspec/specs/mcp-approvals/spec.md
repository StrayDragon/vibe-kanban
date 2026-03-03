# mcp-approvals Specification

## Purpose
为外部编排器（OpenClaw 类）提供纯 MCP 的 approvals 闭环，并确保响应审计字段在“外部编排器弹窗批准”的场景下可用且一致。

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
The system SHALL expose an MCP tool to respond to an approval with `status` in `{approved, denied}` (and optional denial `denial_reason`). The tool SHALL support `request_id` for safe retries.

#### Scenario: Approve a pending approval
- **WHEN** a client calls `respond_approval` for a pending approval with `status=approved`
- **THEN** the approval transitions to `approved` and the waiting executor is unblocked

#### Scenario: Deny with a reason
- **WHEN** a client calls `respond_approval` with `status=denied` and a `denial_reason`
- **THEN** the approval transitions to `denied` and the denial reason is recorded

#### Scenario: Respond is idempotent
- **WHEN** a client retries the same `respond_approval` call with the same `request_id`
- **THEN** the server returns the previously recorded result without applying a duplicate transition

### Requirement: Approvals tools SHALL return structuredContent
系统 SHALL 为 approvals 相关 tools（`list_approvals/get_approval/respond_approval`）返回 `structuredContent`，并确保包含渲染审批 UI 所需的字段（tool_name/tool_input/tool_call_id/timestamps 等）。

#### Scenario: Approval details are machine-readable
- **WHEN** 客户端调用 `get_approval`
- **THEN** 返回包含 `structuredContent`，且其中包含 `tool_name` 与 `tool_input`

### Requirement: Approval responses SHOULD capture responder identity
系统 SHOULD 支持在 approvals 响应中记录响应方身份（例如 `responded_by_client_id`），以支持审计与“外部编排器弹窗批准”的场景。

当客户端未提供 `responded_by_client_id` 时，系统 SHALL 从 MCP peer info 派生默认值并写入持久化存储。

#### Scenario: Responder identity is stored
- **WHEN** 客户端调用 `respond_approval` 并提供 `responded_by_client_id`
- **THEN** 该字段被持久化并可在后续 `get_approval` 中读取

#### Scenario: Responder identity is derived when omitted
- **WHEN** 客户端调用 `respond_approval` 且未提供 `responded_by_client_id`
- **THEN** 系统派生并持久化一个默认 `responded_by_client_id`（例如基于 MCP client name/version）

### Requirement: Approvals MAY use elicitation when supported by client
如果客户端声明支持 MCP elicitation capability，系统 MAY 在 approvals 创建后主动发起 elicitation 请求以收集用户输入；无论是否启用 elicitation，系统 SHALL 保持 pull 闭环（`list/get/respond_approval`）可用。

#### Scenario: Elicitation is optional and has pull fallback
- **WHEN** 客户端不支持 elicitation 或 elicitation 超时
- **THEN** 客户端仍可通过 `list_approvals/get_approval/respond_approval` 完成审批闭环
