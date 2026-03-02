# mcp-task-tools Specification

## Purpose
将 MCP control plane 的 tool 输出与元信息对齐 MCP 结构化输出最佳实践，使外部编排器可以可靠地基于 schema 消费结果、处理错误并进行自动化编排。

## ADDED Requirements

### Requirement: MCP tools SHALL return structuredContent for JSON results
对于返回 JSON 的 MCP tools，系统 SHALL 在 tool result 中提供 `structuredContent`，并将其视为客户端机器消费的主通道。

#### Scenario: Structured output is present
- **WHEN** 客户端调用任一返回 JSON 的 tool（例如 `tail_attempt_feed` 或 `list_tasks`）
- **THEN** 返回结果包含非空 `structuredContent`，且其语义与 tool 文档一致

### Requirement: Tools SHOULD publish outputSchema for JSON results
对于返回 JSON 的 MCP tools，系统 SHOULD 在 tools/list 中提供 `outputSchema`，使客户端可进行 schema 驱动的解析与验证。

#### Scenario: Output schema is discoverable
- **WHEN** 客户端请求 MCP tool 列表
- **THEN** 关键 JSON tools（任务/attempt/approvals/feed 类）在 tool 定义中包含 `outputSchema`

### Requirement: Tools SHALL provide meaningful annotations
系统 SHALL 为 tools 提供 `annotations` 提示信息，至少覆盖：
- 只读工具标记 `readOnlyHint=true`
- 可能破坏性操作标记 `destructiveHint=true`
- 声明支持 `request_id` 幂等的写操作标记 `idempotentHint=true`

#### Scenario: Tools include annotations
- **WHEN** 客户端请求 MCP tool 列表
- **THEN** `annotations` 字段存在且与工具行为一致（只读/破坏性/幂等提示不互相矛盾）

