# api-error-model Specification

## Purpose
定义统一的 API 错误模型：既覆盖 HTTP API 的 `ApiError` → status 映射，也覆盖 MCP 编排场景下的稳定结构化错误码约定（例如 attempt control lease 与 long-poll）。

## Requirements
### Requirement: Consistent error status mapping
The system SHALL map `ApiError` variants to HTTP status codes and SHALL return an `ApiResponse` error payload under a non-200 status.

#### Scenario: BadRequest status
- **WHEN** input validation fails
- **THEN** the response status is `400` and the body is an `ApiResponse` error payload

#### Scenario: Unauthorized status
- **WHEN** a request is missing required authentication
- **THEN** the response status is `401` and the body is an `ApiResponse` error payload

#### Scenario: Forbidden status
- **WHEN** a request is authenticated but not permitted
- **THEN** the response status is `403` and the body is an `ApiResponse` error payload

#### Scenario: NotFound status
- **WHEN** the requested resource does not exist
- **THEN** the response status is `404` and the body is an `ApiResponse` error payload

#### Scenario: Conflict status
- **WHEN** the request conflicts with existing state
- **THEN** the response status is `409` and the body is an `ApiResponse` error payload

#### Scenario: Internal server error status
- **WHEN** an unexpected server error occurs
- **THEN** the response status is `500` and the body is an `ApiResponse` error payload

## ADDED Requirements (MCP)

### Requirement: Business failures SHOULD be returned as structured tool errors
对于可恢复或业务语义明确的失败（例如混用分页、幂等冲突、guardrails 阻断），系统 SHOULD 返回 tool-level error（`isError=true`）并提供结构化错误对象，至少包含：
- `code`（稳定字符串）
- `retryable`（布尔值）
- `hint`（下一步建议）
- `details`（上下文）

#### Scenario: Tool errors are structured
- **WHEN** 客户端触发一次业务失败（例如混用分页）
- **THEN** 返回 `isError=true` 且错误内容包含 `code/hint`，客户端无需解析字符串 JSON 即可编排恢复动作

### Requirement: Attempt control and long-poll failures SHALL use stable error codes
当 MCP 工具调用在语义上合法但触发业务约束时，系统 SHALL 返回 tool-level structured error（`isError=true` 且 `structuredContent` 为对象），并使用稳定 `code` 以支持编排器策略。

本系统新增/固化以下错误码（非穷尽）：
- `attempt_claim_required`：请求的写操作需要有效 lease，但当前无有效 lease。
- `attempt_claim_conflict`：lease 被其他 client 持有且未过期（提示 owner/expires_at）。
- `invalid_control_token`：提供的 `control_token` 不匹配或已过期。
- `mixed_pagination`：同时提供 `cursor` 与 `after_*`（混用分页模式）。
- `wait_ms_too_large`：`wait_ms` 超出服务器允许上限。
- `wait_ms_requires_after_log_index`：`wait_ms` 仅允许与 `after_log_index` 一起使用。

#### Scenario: Invalid control token is structured
- **WHEN** 客户端调用 `send_follow_up` 或 `stop_attempt` 且 `control_token` 无效
- **THEN** tool result 的 `isError=true`，且结构化错误对象包含 `code=invalid_control_token` 与可操作 `hint`

#### Scenario: wait_ms misuse is structured
- **WHEN** 客户端在未提供 `after_log_index` 的情况下调用 `tail_attempt_feed(wait_ms=...)`
- **THEN** tool result 的 `isError=true`，且结构化错误对象包含 `code=wait_ms_requires_after_log_index`
