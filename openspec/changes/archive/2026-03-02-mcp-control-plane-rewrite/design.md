## Context

Vibe Kanban 当前已具备较完善的本地执行与闭环观测能力（attempt 状态、日志、改动、产物、follow-up/stop）。但 MCP 层的实现形态是“对 HTTP API 的转发器”：

- `mcp_task_server` 通过 `reqwest` 调用 `/api/*`，导致每次 tool 调用都产生多跳与重复序列化。
- tool approvals 的响应入口主要通过 HTTP（前端 `PendingApprovalEntry` 调用 `/api/approvals/{id}/respond`），外部编排器无法做到纯 MCP 闭环。
- 人类随时可能打开 UI 查看任务列表、进入某 task/attempt 的日志并接管，因此除了 attempt 级别观测外，需要 project/task 级别的活动与控制面，以保证外部编排器与 UI 对“发生了什么”有一致视图。

本设计将 MCP 作为外部编排器（OpenClaw 类）与 Vibe Kanban 交互的首要接口，目标是显著提升 MCP 利用率与性能，并把 approvals 与 activity feed 纳入 MCP 一等能力。

## Goals / Non-Goals

**Goals:**
- MCP server native：在本地模式下直接初始化 `DeploymentImpl` 并调用 services/db。
- Feed-first：提供 project/task/attempt 的增量 tail 工具，统一 `after_*` 与 `cursor` 规则。
- Approvals-first：DB 持久化 approvals，并提供 MCP list/get/respond，支持“透传给用户的审批交互”。
- 写操作具备幂等（`request_id`），错误码稳定、可恢复。

**Non-Goals:**
- 不做 MCP tool 的向后兼容；不保证旧工具名继续存在。
- 不引入远程鉴权/多用户权限模型。
- 不实现 TaskGroup 自动 runner。
- 不改动前端的信息架构（可为适配 approvals/状态更新做小幅调整）。

## Decisions

1) **MCP server 采用 native 模式（直连 deployment）**
- 方案 A：维持 HTTP 代理（现状）→ 性能与利用率受限。
- 方案 B：native（选择）→ 直接复用 LocalDeployment 与 services/db，减少多跳与重复编解码。
- 方案 C：同时提供两种运行模式 → 增加维护成本；本变更不做兼容承诺，仅在测试中保留对拍能力。

2) **工具面采用“写操作细粒度 + 观测用 feed”混合**
- 写操作（create/update/start/respond）保持单一职责，便于幂等与重试。
- 观测操作用 `tail_*`（project/task/attempt）统一承载“最新变化”，降低编排器拼装成本并提高 MCP 利用率。

3) **Approvals 以 DB 为真实来源，内存仅做 waiter**
- 选择 DB 持久化 pending approvals，保证后端重启后仍可 list/respond，避免 attempt 因内存丢失永久卡住。
- `Approvals` 服务内部保留 “approval_id → waiter” 的短生命周期映射以唤醒正在等待的 executor。

4) **分页与增量语义统一**
- `cursor`：分页获取更旧历史
- `after_*`：增量拉取“新发生的变化”
- 服务器拒绝同时提供 `cursor` 与 `after_*`

5) **幂等语义**
- 所有关键写工具支持 `request_id`，底层使用现有 idempotency key 机制（scope=tool name）确保重试安全。

## Risks / Trade-offs

- [破坏性重写] → 通过 specs 明确工具名/语义，并提供最小示例与测试对拍减少迁移风险。
- [重复能力] `/api/events` 已提供 SSE → MCP 仍需要 project/task feed 以服务外部编排器与接管场景；实现上复用同一事件源与统一 tail 语义。
- [数据增长] approvals 表会累积 → 引入基于 TTL/状态的清理策略（后续可在 cache budgeting/maintenance 中配置）。

## Migration Plan

1) 新增 approvals 表迁移并更新 DB entities/models。
2) 重构 `Approvals` 服务：创建/响应/超时更新同时写 DB；仍向 msg_store 写入 tool status patch。
3) 重写 `mcp_task_server`：native 初始化 deployment，替换全部 tool 实现。
4) 更新 `docs/mcp.md` 并补齐测试（含“对拍 v1”仅用于验证）。

## Open Questions

- project/task feed 的保留窗口与内存上限（事件 ring buffer 容量、TTL）如何配置？
- `responded_by_client_id` 的来源：MCP 客户端是否需要显式传入 `client_id`（用于审计/回放）？

