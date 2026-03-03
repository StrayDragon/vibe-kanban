## 1. 契约验收（先把回归锁住）

- [x] 1.1 扩展 `TaskServer::tool_router_exposes_output_schema_for_key_tools`：改为断言 **全部 26 个 tools** 都有 `outputSchema`（验收：`cargo test -p server mcp::task_server::tests::tool_router_exposes_output_schema_for_key_tools`）
- [x] 1.2 扩展 `TaskServer::tool_router_marks_large_attempt_tools_as_task_optional`：加入 `start_attempt` 的断言（验收：`cargo test -p server mcp::task_server::tests::tool_router_marks_large_attempt_tools_as_task_optional`）

## 2. 补齐 `outputSchema`（tools/list 可机器消费）

- [x] 2.1 为缺失 `outputSchema` 的 tools 批量补齐 `#[tool(output_schema = schema_for_output::<ResponseType>() ...)]`（验收：`cargo test -p server` 且 `tools/list` 中所有 tools 的 `outputSchema != null`）
- [x] 2.2 如遇到缺少 `schemars::JsonSchema` 派生的响应类型，补齐 `#[derive(schemars::JsonSchema)]`（验收：`cargo test -p server`）

## 3. 扩大 `taskSupport`（tasks/* 可选闭环）

- [x] 3.1 为 `start_attempt` 增加 `execution(task_support = \"optional\")`（验收：`cargo test -p server`，且 `tools/list` 里 `start_attempt.execution.taskSupport=optional`）
- [ ] 3.2 评估并决定是否将 `send_follow_up` / `stop_attempt` 也标为 `taskSupport=optional`（基于实测耗时与取消价值）（验收：更新 spec/tests + `cargo test -p server`）

## 4. Inspector 工作流（让外部视角可重复验证）

- [ ] 4.1 在 `docs/mcp.md` 增加 “MCP Inspector（UI/CLI）” 小节：给出 stdio 启动示例（`npx @modelcontextprotocol/inspector -- <mcp_task_server>` / `--cli --method tools/list`），并强调不要关闭 proxy auth（验收：文档可按步骤跑通拿到 tools/list）
- [ ] 4.2 （可选）在 `justfile` 增加 `mcp-inspector` target，封装常用 inspector 命令（验收：`just mcp-inspector` 能启动 inspector 并连接到本地 server）

## 5. 最终验收

- [ ] 5.1 `cargo test --workspace` 通过
- [ ] 5.2 使用 inspector CLI 拉取 `tools/list`，确认：
  - tools 数量/名称符合 `mcp-task-tools` spec
  - 所有 tools 都包含 `outputSchema`
  - `start_attempt/get_attempt_changes/get_attempt_patch/get_attempt_file` 包含 `taskSupport=optional`
