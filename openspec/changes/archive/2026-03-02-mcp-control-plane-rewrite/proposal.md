## Why

当前 `mcp_task_server` 更像是对 `/api` 的“HTTP 代理层”（stdio → MCP → reqwest → HTTP → backend），导致：

- 外部编排器（OpenClaw 类）需要大量回合才能完成一次闭环操作（启动、观测、审批、停止、取产物），MCP 利用率低。
- 关键流程（尤其是 tool approvals）无法通过 MCP 纯闭环完成，必须混用 HTTP 才能响应审批，难以做“委托式/透传式”的交互。
- 人类随时可能进入 UI 查看或接管，因此除了 attempt 级别外，还需要 project/task 级别的活动与控制面以保持一致的可观测性与可操作性。

我们需要把 MCP 从“代理 HTTP”升级为本地优先的 **Control Plane**：外部编排器以 MCP 为唯一接口即可完成从创建任务到闭环审查的全流程，并且在性能与可靠性上可承载高频使用。

## Goals

- 将 MCP 重写为 **native 模式**：直接调用本地 `DeploymentImpl`（LocalDeployment）与 services/db，避免 HTTP 往返。
- 以 **Feed-first** 的方式暴露活动与日志：提供 project/task/attempt 级别的增量 tail 工具，使外部编排器与人类接管场景都能高效获取“最新变化”。
- 将 **tool approvals** 作为一等资源纳入 MCP：支持 list/get/respond，并在后端重启后可恢复（持久化）。
- 将“启动 attempt + 首条消息发送”收敛为更少回合，提升编排器闭环效率并减少 `no_session_yet` 类重试。
- 明确且一致的错误码、分页与幂等语义，便于编排器稳定实现。

## Non-Goals

- 不提供对现有 MCP 客户端的向后兼容（这是一次破坏性重写）。
- 不在本变更中引入远程多用户鉴权/权限体系（可在后续独立 capability 中处理）。
- 不实现 TaskGroup/DAG 自动 runner（仍主要面向人类 UI；编排器可在更高层实现调度）。
- 不对前端 UI 做大规模重构（允许为配合后端变更做小幅适配）。

## What Changes

- **BREAKING**：重写 MCP tool 集合与语义，围绕 project/task/attempt + approvals + feed 重新设计。
- `mcp_task_server` 改为 native（不再通过 reqwest 调用 `/api`）。
- 新增 approvals 数据模型与迁移：tool approvals 持久化（支持重启恢复）。
- 重构后端 approvals 服务：DB 为真实来源，内存仅用于等待/唤醒 executor。
- 新增/强化 tail 工具（project/task/attempt 级别）以支撑高频观测与人类接管。
- 更新 `docs/mcp.md` 与相关测试用例。

## Capabilities

### New Capabilities
- `mcp-activity-feed`: 定义 project/task/attempt 级别活动与日志的增量拉取语义（cursor/after_* 规则一致）。
- `mcp-approvals`: 定义 approvals 的持久化、列出、获取详情与响应语义（含幂等与并发保证）。

### Modified Capabilities
- `mcp-task-tools`: MCP tool 集合与闭环工作流重设计（新增 feed/approvals；替换启动与 follow-up 流程；统一分页与错误码）。

## Impact

- Rust：
  - MCP：`crates/server/src/mcp/task_server.rs`、`crates/server/src/bin/mcp_task_server.rs`
  - Approvals：`crates/services/src/services/approvals.rs`、`crates/services/src/services/approvals/executor_approvals.rs`
  - DB：`crates/db/migration/src/*`、`crates/db/src/entities/*`、`crates/db/src/models/*`
- Docs：`docs/mcp.md`
- Tests：MCP server tests、approvals/service tests、必要时对拍测试（仅用于验证，不提供兼容承诺）

## Risks

- [破坏性变更] 外部编排器需要同步升级 → 通过清晰的 spec、稳定错误码与测试样例降低迁移成本。
- [复杂度上升] MCP 直接持有 deployment，调用链更深 → 通过严格边界（仅暴露必要 tool）与测试覆盖控制。
- [审批可靠性] approvals 持久化带来迁移与一致性问题 → 以 DB 作为单一真实来源，内存仅做 waiter，提供幂等 respond。

## Verification

- `cargo test --workspace` 覆盖：
  - approvals 持久化 + 重启恢复 + respond 唤醒 executor 的闭环
  - MCP `tail_*` 的增量拉取与分页互斥约束
  - MCP 启动 attempt + prompt 的单回合闭环（无 `no_session_yet`）
- 手动 smoke：
  - 通过外部编排器（或最小脚本）仅用 MCP 完成：创建任务 → 启动 attempt → 拉取 feed → 处理审批 → 获取 changes/patch → stop

