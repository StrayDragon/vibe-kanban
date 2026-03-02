# api-error-model Specification

## Purpose
补齐 MCP 工具调用场景下的错误返回约定，使编排器能够稳定区分“参数错误”和“业务错误”，并以一致的字段实现重试/恢复/提示。

## ADDED Requirements

### Requirement: Business failures SHOULD be returned as structured tool errors
对于可恢复或业务语义明确的失败（例如混用分页、幂等冲突、guardrails 阻断），系统 SHOULD 返回 tool-level error（`isError=true`）并提供结构化错误对象，至少包含：
- `code`（稳定字符串）
- `retryable`（布尔值）
- `hint`（下一步建议）
- `details`（上下文）

#### Scenario: Tool errors are structured
- **WHEN** 客户端触发一次业务失败（例如混用分页）
- **THEN** 返回 `isError=true` 且错误内容包含 `code/hint`，客户端无需解析字符串 JSON 即可编排恢复动作

