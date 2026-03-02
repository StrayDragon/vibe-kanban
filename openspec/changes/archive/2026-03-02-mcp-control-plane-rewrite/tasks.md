## 1. 数据模型（Approvals 持久化）

- [x] 1.1 新增 `approvals` 表迁移（含 attempt_id、execution_process_id、tool_call_id、status、timestamps）并运行 `pnpm run prepare-db` 验证
- [x] 1.2 添加 SeaORM entity/model（`crates/db/src/entities/approval.rs`、`crates/db/src/models/approval.rs`）并为常用查询加索引验证

## 2. Approvals 服务重构（DB 真源 + 内存 waiter）

- [x] 2.1 重构 `crates/services/src/services/approvals.rs`：create/list/get/respond/timeout 全部写 DB，内存仅用于 waiter 唤醒（`cargo test --workspace -p services`）
- [x] 2.2 保持现有对话 patch 行为：pending/approved/denied/timed_out 状态仍写入 MsgStore（UI smoke：触发一次 tool approval 并能在 UI 响应）

## 3. MCP server native 重写（破坏性替换）

- [x] 3.1 改写 `crates/server/src/bin/mcp_task_server.rs`：直接初始化 `DeploymentImpl`，不再依赖 `/api` 与端口文件（`cargo run --bin mcp_task_server` 能启动）
- [x] 3.2 重写 `crates/server/src/mcp/task_server.rs`：移除 reqwest/base_url，实现新 tool 集合与 schema 约束（`cargo test --workspace -p server`）
- [x] 3.3 实现 `start_attempt`（可携带 `prompt`）与 `send_follow_up`（含 idempotency），并提供 `tail_attempt_feed` 引导消除 `no_session_yet` 依赖（测试覆盖）

## 4. Feed 与活动 tail（project/task/attempt）

- [x] 4.1 实现 `tail_attempt_feed`：增量拉取最新 normalized logs + pending approvals 摘要（测试：after_log_index 仅返回新条目）
- [x] 4.2 实现 `tail_project_activity`/`tail_task_activity`：统一 after_event_id/cursor 语义并拒绝混用

## 5. MCP Approvals 工具闭环

- [x] 5.1 实现 `list_approvals`/`get_approval`/`respond_approval` 并与 executor waiter 联动（测试：respond 解除阻塞）
- [x] 5.2 增加“重启恢复”测试：pending approval 在重启后仍可 list/respond（集成测试）

## 6. 文档与回归

- [x] 6.1 更新 `docs/mcp.md`：用新工具名给出 OpenClaw 推荐闭环链路
- [x] 6.2 全量回归：`cargo test --workspace`
