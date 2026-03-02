## Why

当前 Vibe Kanban 的 MCP 已经可以作为外部编排器（OpenClaw 类）驱动项目的 control plane，但在“代理用户做操作”的场景里仍存在两类摩擦：

1) **高频拉取带来的性能/延迟问题**：编排器为了低延迟体验，通常会高频调用 `tail_*` 工具轮询新日志/事件。
2) **多方接管/并发驱动的竞态**：同一个 attempt 可能同时被不同“上层代理”（自动编排、人类接管、脚本化修复）发起 follow-up/stop，导致指令互相覆盖或执行顺序不可控。

本变更引入“长轮询 tail”与“attempt 控制租约（lease）”，把这些竞争与性能问题收敛到 MCP 层的可编排语义内。

### Goals
- 外部编排器以更低调用频率获得接近实时的 attempt 进展（降低轮询开销与延迟抖动）。
- 为 attempt 引入可选/可强制的控制租约语义，支持自动编排与人类接管之间的显式交接。
- 所有新语义都通过结构化输出与稳定错误码表达，便于编排器做策略（重试、降级、接管）。

### Non-goals
- 不在本变更中升级 `rmcp` 或切换 MCP protocol version（升级作为后续提案单独推进）。
- 不实现 MCP 层的服务端持续推送（notifications/streaming）；仍以 request/response +（可选）长轮询为主。

### Risks
- 长轮询会占用连接与部分服务端资源；需限制 `wait_ms` 上限并确保超时可控。
- 租约机制如果缺少 TTL/续租策略，客户端崩溃可能导致“假锁”；需要 TTL 与可抢占/接管路径。

### Verification
- 为 `tail_attempt_feed(wait_ms)` 增加集成测试：无新日志时等待后返回、出现新日志时提前返回。
- 为 lease 工具增加测试：claim/renew/release/冲突与错误码。
- `cargo test -p server` 通过，且关键 MCP 工具在 `tools/list` 中 schema/annotations 保持可消费。

## What Changes

- `tail_attempt_feed` 增加可选参数 `wait_ms`（仅在 `after_log_index` 模式生效）：当没有新日志/approval 变化时，最多等待 `wait_ms` 毫秒后返回，用于编排器“低频调用 + 低延迟体验”。
- 引入 attempt 控制租约（lease）工具集：
  - 新增：`claim_attempt_control`、`get_attempt_control`、`release_attempt_control`。
  - `start_attempt` 在成功创建 attempt 后默认授予调用方一个 `control_token`（带 TTL）。
  - **BREAKING（可选）**：`send_follow_up` / `stop_attempt` 增加 `control_token` 校验（默认开启），避免并发驱动竞态；在需要兼容时可通过配置开关或提供 v2 工具绕过。
- `respond_approval`：当 `responded_by_client_id` 为空时，默认使用 MCP peer info 派生（例如 `mcp:<client>@<version>`），提升审计一致性。
- 增加/固化 structured tool error 约定：
  - `attempt_claim_conflict` / `attempt_claim_required` / `invalid_control_token` / `wait_ms_too_large` 等。
- 更新文档与 OpenSpec 规范：补充长轮询与 lease 的调用范式、错误码与推荐编排策略。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `mcp-task-tools`: 增加 attempt lease 工具；`start_attempt`/`send_follow_up`/`stop_attempt` 增加 lease 相关字段与行为约束。
- `mcp-activity-feed`: `tail_attempt_feed` 增加 `wait_ms` 长轮询语义与限制。
- `mcp-approvals`: `respond_approval` 的 `responded_by_client_id` 默认派生与审计语义更新。
- `api-error-model`: 新增 MCP 场景下 lease/long-poll 相关的稳定错误码与重试提示约定。

## Impact

- 后端：主要改动在 `crates/server/src/mcp/task_server.rs`（新增 tools/参数/校验/错误码），以及 `crates/utils/src/msg_store.rs`（暴露可等待的新日志事件 receiver，支持长轮询实现）。
- 外部编排器（OpenClaw）：需要在 `start_attempt` 后保存 `control_token` 并在 follow-up/stop 时携带；可用 `wait_ms` 降低轮询频率。
- 规范与文档：更新 `docs/mcp.md` 与 OpenSpec delta specs。

