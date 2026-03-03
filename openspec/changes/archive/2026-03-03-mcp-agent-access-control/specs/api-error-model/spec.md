# api-error-model Specification

## Purpose
补充 MCP 编排场景下（attempt control lease + long-poll）的稳定错误码约定，使外部编排器可以可靠地恢复与接管。

## ADDED Requirements (MCP)

### Requirement: Attempt control and long-poll failures SHALL use stable error codes
当 MCP 工具调用在语义上合法但触发业务约束时，系统 SHALL 返回 tool-level structured error（`isError=true` 且 `structuredContent` 为对象），并使用稳定 `code` 以支持编排器策略。

本变更新增/固化以下错误码（非穷尽）：
- `attempt_claim_required`：请求的写操作需要有效 lease，但当前无有效 lease。
- `attempt_claim_conflict`：lease 被其他 client 持有且未过期（提示 owner/expires_at）。
- `invalid_control_token`：提供的 `control_token` 不匹配或已过期。
- `wait_ms_too_large`：`wait_ms` 超出服务器允许上限。
- `wait_ms_requires_after_log_index`：`wait_ms` 仅允许与 `after_log_index` 一起使用。

#### Scenario: Invalid control token is structured
- **WHEN** 客户端调用 `send_follow_up` 或 `stop_attempt` 且 `control_token` 无效
- **THEN** tool result 的 `isError=true`，且结构化错误对象包含 `code=invalid_control_token` 与可操作 `hint`

#### Scenario: wait_ms misuse is structured
- **WHEN** 客户端在未提供 `after_log_index` 的情况下调用 `tail_attempt_feed(wait_ms=...)`
- **THEN** tool result 的 `isError=true`，且结构化错误对象包含 `code=wait_ms_requires_after_log_index`
