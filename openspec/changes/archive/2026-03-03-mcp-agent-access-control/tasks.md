## 1. 数据模型与迁移

- [x] 1.1 新增 DB migration：创建 `attempt_control_leases` 表并注册到 `crates/db/migration/src/lib.rs`（验证：`pnpm run prepare-db` 或 `cargo test -p db`/`cargo test --workspace` 迁移可运行）
- [x] 1.2 添加 SeaORM entity：`crates/db/src/entities/attempt_control_lease.rs` + 在 `crates/db/src/entities/mod.rs` 导出（验证：`cargo test -p db` 编译通过）
- [x] 1.3 添加 DB model API：`crates/db/src/models/attempt_control_lease.rs`（get/claim/release/ttl 判断）（验证：为模型增加至少 1 个测试或在 server tests 覆盖）

## 2. Long-poll 基础能力

- [x] 2.1 为 `MsgStore` 增加 normalized log entry 的订阅接口（用于等待新日志）。（验证：`cargo test -p utils` 或 `cargo test --workspace`）

## 3. MCP：attempt feed 长轮询

- [x] 3.1 扩展 `TailAttemptFeedRequest` 增加 `wait_ms`，并在 `tail_attempt_feed` 中实现长轮询（仅 after 模式生效，含上限与 structured error）。（验证：新增 server 测试覆盖 wait_ms 正常等待与参数误用）

## 4. MCP：attempt 控制租约（lease）

- [x] 4.1 新增 MCP tools：`claim_attempt_control` / `get_attempt_control` / `release_attempt_control`（结构化输出 + 稳定错误码）。（验证：`cargo test -p server` 覆盖 claim/冲突/释放）
- [x] 4.2 `start_attempt` 创建默认 lease 并返回 `control_token`。（验证：server 测试断言响应包含 token 且后续可用）
- [x] 4.3 `send_follow_up` / `stop_attempt` 增加 `control_token` 校验并返回结构化错误（`attempt_claim_required/attempt_claim_conflict/invalid_control_token`）。（验证：server 测试覆盖无 token/错 token/冲突）

## 5. MCP：approvals 审计默认值

- [x] 5.1 `respond_approval` 在未提供 `responded_by_client_id` 时从 MCP peer info 派生默认值并持久化。（验证：新增/更新 server 测试断言字段被写入）

## 6. 文档与回归

- [x] 6.1 更新 `docs/mcp.md`：补充 `tail_attempt_feed(wait_ms)` 与 `control_token` 的推荐编排流程与错误码。（验证：文档示例与实际工具入参一致）
- [x] 6.2 回归：`cargo test -p server` + `cargo test --workspace`。（验证：CI 命令本地通过）
