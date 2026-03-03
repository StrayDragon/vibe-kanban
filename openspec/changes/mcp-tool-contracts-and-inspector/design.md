## Context

当前 `mcp_task_server` 基于 `rmcp`，对外宣告的协议版本为 `ProtocolVersion::LATEST`（在 `rmcp 0.17.x` 中解析为 `2025-06-18`），并启用 capabilities：`tools` + `tasks`。MCP tools 已覆盖 Vibe Kanban 的核心闭环（project/task/attempt + approvals + feed + changes），但 `tools/list` 元数据仍存在两个明显缺口：

1) **`outputSchema` 覆盖不足**：只有少数工具在 `tools/list` 中包含 `outputSchema`，导致外部编排器无法做 schema 驱动的参数/返回值校验与 UI 生成（必须硬编码）。

2) **`taskSupport` 标注不足**：仅部分重 IO 工具标了 `taskSupport=optional`，但 `start_attempt` 这类“启动链路慢/可取消/可能跨重启观察”的调用未纳入 tasks 体系，编排器难以做一致的异步调度与取消。

同时，我们希望把 “工具契约质量” 变成可重复验证的工程资产，而不是靠人肉对拍。`@modelcontextprotocol/inspector`（UI/CLI）是一个合适的外部视角验收工具：它能用标准 MCP handshake 连接 server，并直接展示 `tools/list` 的 schema/annotations/taskSupport。

## Goals / Non-Goals

**Goals:**
- `tools/list` 对 **全部 MCP tools（本项目声明的 26 个）** 提供稳定可消费的契约信息：
  - 所有返回 JSON 的工具都暴露 `outputSchema`，并与成功返回的 `structuredContent` 结构一致。
  - `annotations` 与实际语义一致（readOnly / destructive / idempotent）。
- 扩大 `taskSupport` 标注，使编排器可选择对慢工具走 `tasks/*`：
  - 至少覆盖 `start_attempt`（并基于实测决定是否包含 `send_follow_up` / `stop_attempt` 等）。
- 提供 inspector 驱动的验收方式（文档 + 可脚本化的最小 smoke test），用于开发调试与未来 CI/回归。

**Non-Goals:**
- 不引入新的 transport（仍以 stdio 为主；不强推 streamable HTTP / SSE）。
- 不改变既有 tool 的业务语义（例如字段含义、状态机、分页语义）。
- 不做大规模错误模型重构（仅在需要时补齐最小一致性约束）。

## Decisions

1) **输出 schema 的实现策略：优先“补齐 `#[tool(output_schema=...)]`”，保留现有 `CallToolResult` 风格**
   - 现状：多数 tools 通过自定义 `CallToolResult { structuredContent + pretty text }` 返回，便于人类阅读与日志检查。
   - 选择：为每个 tool 的成功响应类型生成 schema（`rmcp::handler::server::tool::schema_for_output::<T>()`）并注入到 `#[tool(output_schema = ...)]`，避免切换到 `rmcp::Json<T>` 带来的输出文本格式变化与潜在兼容性风险。
   - 例外：已经使用 `rmcp::Json<T>` 的工具保持不变。

2) **taskSupport：从“重 IO”扩展到“启动链路”**
   - 将 `start_attempt` 标记为 `taskSupport=optional`，使 OpenClaw 类编排器可用 `tasks/*` 做：
     - 可取消（`tasks/cancel` → server 侧尊重 cancellation token）
     - 可重启恢复观察（重启后仍可 `tasks/get` / `tasks/result`）
   - 是否扩展到 `send_follow_up` / `stop_attempt` 由实际耗时与失败模式决定（以不增加编排复杂度为原则）。

3) **Inspector 验收：以外部客户端视角约束契约质量**
   - 增加文档与命令示例（UI 与 `--cli`），让任何人都能在本地一键看到：
     - tools 数量与名称是否符合 spec
     - 每个 tool 是否有 `outputSchema` / `annotations` / `taskSupport`
   - 允许后续将 inspector CLI 集成到 CI（作为可选 job），但本变更先以文档 + 本地脚本为主，避免引入 Node 版本/环境耦合导致 CI 不稳定。

## Risks / Trade-offs

- [批量改动工具宏注解] → 通过脚本化检查（inspector CLI 或内部测试）确保 `tools/list` 结构稳定；每次改动聚焦于 schema/metadata，不改业务逻辑。
- [schema 与真实输出不一致] → 在关键 tools 上补充单测/快照测试：调用 tool 并用 `schemars` 生成的 schema 校验 `structuredContent`（至少覆盖 approvals / start_attempt / changes 三类）。
- [taskSupport 扩展带来编排差异] → `taskSupport=optional`（而非 required），让客户端可渐进启用；并在文档里给出推荐策略（启动链路优先 tasks，轻量工具直接 call）。

## Migration Plan

- 纯后端变更（工具元数据 + 轻量脚本/文档），不涉及 DB migration。
- 回滚策略：回滚 commit 即可；不改变存量数据结构。

## Open Questions

- 是否在本变更内引入一个标准化的 tool-error schema（成功/失败的 `structuredContent` 统一 envelope），并把 `outputSchema` 定义成 `anyOf(success, error)`？（可能提升强类型消费体验，但会扩大变更范围）
- `stop_attempt` 是否需要补充 `request_id` 以匹配 `idempotentHint=true` 的语义一致性？（可作为后续小变更）

