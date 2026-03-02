## 1. 结构化输出基建（structuredContent）

- [ ] 1.1 统一 MCP 成功/失败返回：为 `crates/server/src/mcp/task_server.rs` 增加结构化输出 helper（success / structured_error），并保持 `content` 仍包含可读 JSON 文本（验证：最少 3 个 tool 端到端返回包含 `structuredContent`）
- [ ] 1.2 为 `tail_attempt_feed` 增量分页错误（mixed pagination）改为结构化 tool error（验证：对应测试断言 `isError=true` 且错误对象含 `code/hint`）

## 2. outputSchema（Json<T>）覆盖关键 tools

- [ ] 2.1 将只读核心 tools 迁移为 `rmcp::Json<ResponseType>`（至少：`list_projects`、`list_tasks`、`tail_attempt_feed`）（验证：tools/list 中出现 `outputSchema`）
- [ ] 2.2 处理非 object 输出的 schema 约束：必要时将数组/标量输出包裹为对象（验证：schema root type 为 object）
- [ ] 2.3 更新/新增 MCP 结构化输出测试（验证：`cargo test --workspace -p server`）

## 3. Tool annotations（readOnly/idempotent/destructive）

- [ ] 3.1 为所有只读 tools 标注 `readOnlyHint=true`（验证：tools/list 中 annotations 存在）
- [ ] 3.2 为写入类 tools 标注 `idempotentHint/destructiveHint`（例如：`create_task/start_attempt/respond_approval/stop_attempt/delete_task`）（验证：抽样检查 5 个工具的 annotations）

## 4. 错误模型对齐（api-error-model）

- [ ] 4.1 定义并落地统一错误对象结构（`code/retryable/hint/details`），并在可恢复业务失败中使用（验证：至少覆盖 mixed pagination、guardrails blocked、idempotency conflict 三类）
- [ ] 4.2 文档化 MCP 错误分层：参数错误（JSON-RPC）vs 业务错误（tool structured_error）（验证：更新 `docs/mcp.md` 并给出示例）

## 5. Approvals elicitation（可选 push，加速交互）

- [ ] 5.1 增加 capability 探测：仅当 client 声明支持 elicitation 时启用 push（验证：无 capability 时行为不变）
- [ ] 5.2 在 approval 创建时发起 elicitation，并将响应映射为 `respond_approval`（验证：集成测试模拟 approve/deny）
- [ ] 5.3 确保 pull 闭环不受影响（验证：`list/get/respond_approval` 仍可完成审批）

## 6. 回归与交付

- [ ] 6.1 更新 OpenSpec 主规范（如需要，将本 change 的 specs 同步到 `openspec/specs/*`）
- [ ] 6.2 全量回归：`cargo test --workspace`、`cargo clippy --workspace --all-targets`（验证：CI 预期全绿）
