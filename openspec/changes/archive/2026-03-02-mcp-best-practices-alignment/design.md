## Context

在 `mcp-control-plane-rewrite` 之后，`mcp_task_server` 已经成为外部编排器的 control plane（stdio MCP，feed-first tail、approvals 持久化等）。

但目前 MCP tool 的输出形态仍以 `Content::text(JSON 字符串)` 为主（客户端需要自己解析字符串 JSON），且 tool 定义未充分利用 MCP/SDK 已提供的能力：

- `CallToolResult.structuredContent` 与 `Tool.outputSchema`（结构化输出与 schema）
- `Tool.annotations`（readOnly/idempotent/destructive/openWorld 的提示）
- （可选）`elicitation/create`：服务器向客户端请求用户输入的标准交互机制

这使得 OpenClaw 类编排器在“稳定解析 / 错误处理 / 自动化审批交互 / 高性能轮询”上仍有额外成本。

## Goals / Non-Goals

**Goals:**
- MCP tools 以 `structuredContent` 作为机器消费的主通道，并尽量为 tools 暴露 `outputSchema`。
- 形成统一的错误结构与 code 约定，便于编排器重试与恢复（减少混用 JSON-RPC error 与 tool-level error 的情况）。
- 为 tools 增加 `annotations`，便于编排器做策略（只读、破坏性、幂等等）。
- approvals 交互在保持 pull（`list/get/respond_approval`）闭环的同时，支持可选的 elicitation push（客户端声明支持时启用）。

**Non-Goals:**
- 不在本变更中引入/实现 MCP experimental `tasks` 全链路（可作为后续独立 change）。
- 不重做现有的 HTTP/SSE/WS 观测能力；MCP 仍以“短请求 + 分页/增量”作为主范式。
- 不改变 kanban 的核心领域模型（project/task/attempt/workspace/session/execution_process）。

## Decisions

1) **结构化输出的落地方式：优先使用 rmcp `Json<T>` 返回类型**

- 方案 A：保持 tool 返回 `CallToolResult`，把成功/失败都改用 `CallToolResult::structured/structured_error`
  - 优点：侵入小、可快速全量覆盖
  - 缺点：不会自动生成 `outputSchema`；需要为每个 tool 显式声明 `output_schema` 才能完全对齐最佳实践
- 方案 B（选用）：将成功返回改为 `Result<Json<ResponseType>, ErrorData>` 或等价方式，让 SDK 自动提供 `outputSchema`
  - 优点：schema 自动生成/校验路径更清晰；客户端可直接依赖 `outputSchema + structuredContent`
  - 风险：需要较大范围的函数签名调整；需要明确“哪些错误用 JSON-RPC error，哪些用 tool-level error”

决定：采用方案 B，并允许在过渡期对少量复杂工具保留 `CallToolResult`，但输出也必须具备 `structuredContent`。

2) **错误语义：参数错误走 JSON-RPC，业务错误走结构化 tool error**

- 参数校验/缺参/类型不匹配：继续使用 `ErrorData::invalid_params`（JSON-RPC error），data 里附带 path/hint。
- 业务层可恢复错误（not found / blocked guardrails / idempotency conflict / mixed pagination）：返回 `CallToolResult::structured_error`，包含：
  - `code`（稳定字符串）
  - `retryable`（bool）
  - `hint`（给编排器的下一步）
  - `details`（结构化上下文）

这样客户端可以统一用 `isError=true + structuredContent` 处理业务失败，而 JSON-RPC error 主要用于“调用不合法”。

3) **Tools annotations：显式标注读写/幂等/破坏性**

为关键 tools 增加 `ToolAnnotations`：
- `readOnlyHint=true`：list/get/tail 类
- `readOnlyHint=false` 且 `idempotentHint=true`：支持 `request_id` 幂等的写操作（如 create_task/start_attempt/respond_approval）
- `destructiveHint=true`：delete/stop/force 等
- `openWorldHint=false`：大部分 control plane 操作是“闭世界”；涉及 git/文件系统/外部依赖探测的工具可标注为 true

4) **Approvals：pull 闭环为默认，elicitation 为可选加速通道**

- 默认：编排器轮询 `tail_attempt_feed` / `list_approvals`，再 `respond_approval`。
- 可选：当客户端在 initialize 中声明支持 `elicitation` capability 时，服务端在创建 approval 时发送 `elicitation/create` 请求，包含 tool_name/tool_input/timeout，并在客户端响应后调用现有 approvals 服务完成落库与唤醒。
- 回退：客户端不支持或超时 → 仍可通过 `list/get/respond_approval` 完成闭环。

## Risks / Trade-offs

- **兼容性风险**：部分客户端可能依赖“pretty JSON 文本”。→ 缓解：保留 `content` 文本为 JSON（并尽量保持可读），新增 `structuredContent` 作为机器通道。
- **实现侵入性**：大量工具签名/返回类型需要调整。→ 缓解：分阶段迁移（先 structuredContent，再 outputSchema/annotations）。
- **错误语义分裂**：JSON-RPC 与 tool error 同时存在。→ 缓解：明确分工（参数错误=JSON-RPC；业务错误=tool structured_error），并在 `docs/mcp.md` 中固化。
- **elicitation 安全/UX**：服务端 push 交互需要客户端妥善呈现。→ 缓解：仅在 client capability 声明时启用，并保留 pull 兜底。

## Migration Plan

1. 全量工具返回补齐 `structuredContent`（保持现有行为不变，优先不破坏）。
2. 分批将工具成功返回切换为 `Json<T>` 并补齐 `outputSchema`。
3. 引入 tool annotations（不影响行为，仅提升可观测/策略）。
4. approvals 增强：在可用时引入 elicitation push，并保持 pull 兜底。
5. 更新与回归：`cargo test --workspace`，并补充 MCP 端到端用例（结构化输出与错误语义）。

## Open Questions

- 是否需要在 MCP 层引入统一的 error code namespace（例如 `vk/<domain>/<code>`），避免与第三方 code 冲突？
- 对于“幂等进行中”的返回，是更适合 tool structured_error 还是 JSON-RPC error？
- elicitation 的 payload schema（字段命名、超时语义）是否需要单独一份 OpenSpec capability？
