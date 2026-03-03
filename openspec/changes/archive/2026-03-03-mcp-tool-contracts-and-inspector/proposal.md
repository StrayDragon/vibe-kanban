## Why

外部编排器（OpenClaw 类）正在把 Vibe Kanban 的 MCP 当作 control plane 来做“多 agent 并发 + 人类随时接管”。当前 `tools/list` 中大部分工具缺少 `outputSchema`、长耗时/重 IO 工具的 `taskSupport` 覆盖不足，导致编排器需要硬编码解析与重试策略，集成成本高且容易在升级时回归。

## What Changes

- 为 `mcp_task_server` 暴露的 **全部 MCP tools** 补齐可机器消费的契约信息：
  - `tools/list` 中为所有返回 JSON 的工具提供 `outputSchema`（与 `structuredContent` 对齐）。
  - 继续强化 `annotations` 的一致性（readOnly / destructive / idempotent）。
- 扩大 `taskSupport` 覆盖面，使编排器可对“慢/重/可取消”的调用走 `tasks/*` 闭环：
  - 将 `start_attempt`（以及评估后确定的其它慢工具）标为 `taskSupport=optional`。
- 引入可重复的“工具契约验收”工作流：
  - 提供基于 `npx @modelcontextprotocol/inspector`（UI/CLI）的本地检查步骤与脚本化 smoke 测试建议，用于快速发现 `outputSchema`/annotations/taskSupport 回归。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `mcp-task-tools`: 将 “tools SHOULD publish outputSchema” 提升为更强约束（覆盖整个工具集），并补充关于 `taskSupport` 标注与 inspector 可验证性的要求。

## Impact

- Backend（Rust）：`crates/server/src/mcp/task_server.rs` 中 MCP tools 的返回类型/宏注解会发生批量调整；必要时会补充少量共享 schema 类型（用于工具成功/错误结构）。
- 外部编排器（OpenClaw）：可依赖 `outputSchema` 进行 schema 驱动的参数/结果处理；可选择对慢工具走 `tasks/*`（可轮询、可取消、可重启恢复）。
- 开发体验：新增 inspector 运行指引/脚本后，工具契约变更将更易被发现与回归验证。

