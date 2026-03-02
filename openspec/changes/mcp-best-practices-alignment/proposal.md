## Why

当前 `mcp_task_server` 的 tool 返回主要依赖 `Content::text(JSON 字符串)`，外部编排器（OpenClaw 类）必须做“字符串 JSON 解析”，难以利用 MCP 的 `structuredContent/outputSchema`、也不利于稳定的错误处理与自动化交互（尤其是 approvals 场景）。

我们希望把 MCP 当作唯一 control plane 接口，因此需要将现有实现对齐 MCP 近期最佳实践：结构化输出、可机器消费的 schema、工具元信息（readOnly/idempotent 等）、以及在 approvals 场景下更标准的“向客户端请求用户输入”的交互模式。

## What Changes

- MCP tool 返回全面升级为 **结构化输出优先**：为绝大多数 tool 增加 `outputSchema` 并返回 `structuredContent`（同时保留可读文本内容用于调试/兼容）。
- 统一错误语义：减少“既用 JSON-RPC error 又在 CallToolResult.error 里塞 JSON 字符串”的混用，形成稳定、可重试、可编排的错误结构与 code 约定。
- 为 tools 增加注解/元信息（`readOnlyHint/destructiveHint/idempotentHint/openWorldHint`），提升通用编排器的策略能力（重试、确认、审计、风险提示）。
- approvals 交互增强（可选）：当客户端支持 MCP elicitation 能力时，支持服务端主动触发“请求用户输入”的交互闭环；不支持时保持 `list/get/respond_approval` 拉取式闭环。
- 更新文档与示例：`docs/mcp.md` 增补 structured output 约定、错误模型与 approvals 交互建议。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `mcp-task-tools`: tool 输出从“文本 JSON”为主升级为 `structuredContent + outputSchema`；增加 tool annotations；补齐一致的错误与 idempotency 约定。
- `mcp-approvals`: approvals 返回与错误结构化；可选引入 elicitation 交互路径；`responded_by_client_id` 等审计字段约定更明确。
- `mcp-activity-feed`: activity/feed 返回结构化与分页语义（cursor/after_* 互斥）进一步固化为可依赖契约。
- `api-error-model`: 补充 MCP 场景下的错误 code/重试语义映射建议（保持与现有 HTTP 错误模型一致）。

## Impact

- 后端：`crates/server/src/mcp/task_server.rs` 的返回类型与工具定义将发生系统性调整；可能需要引入/使用 rmcp 的 `Json<T>` 包装与 schema 生成能力。
- 外部编排器：可以直接读取 `structuredContent`，减少解析与脆弱性；同时也需要更新对错误 code/重试语义的处理。
- UI/运维：`docs/mcp.md` 与 OpenSpec 规范将更新；不改变现有 Kanban 并发开发模型与 attempt/workspace 的核心概念。
