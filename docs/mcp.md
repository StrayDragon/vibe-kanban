# Vibe Kanban MCP（`mcp_task_server`）— Control Plane v2 指南

目标：让外部编排器（OpenClaw 类）以 **MCP 为唯一接口** 完成闭环：**创建任务 → 启动 attempt → 观测日志/处理审批 → 获取改动/产物 → 停止**，并兼顾人类随时进入 UI 查看/接管的场景。

如果只记一条链路：
`start_attempt(prompt?) → tail_attempt_feed(after_log_index) → respond_approval → get_attempt_changes/get_attempt_patch/get_attempt_file → stop_attempt`

## 术语

- `project_id`：项目 id
- `task_id`：任务 id
- `attempt_id`：一次执行的 workspace id（MCP 里统一叫 attempt）
- `session_id`：会话上下文（用于 `send_follow_up` / `tail_session_messages`）
- `execution_process_id`：一次具体执行进程（用于 approvals 绑定）
- `state`：`idle | running | completed | failed`

## 工具清单（v2）

发现/预检：
- `list_projects` / `list_repos(project_id)` / `list_executors` / `cli_dependency_preflight`

任务：
- `list_tasks(project_id, status?, limit?)` / `get_task(task_id)`
- `create_task(project_id, title, description?, request_id?)`
- `update_task(task_id, title?, description?, status?)`
- `delete_task(task_id)`

attempt：
- `list_task_attempts(task_id)`
- `start_attempt(task_id, executor, repos[], variant?, request_id?, prompt?)`
- `send_follow_up({attempt_id|session_id}, control_token, prompt, variant?, request_id?)`
- `stop_attempt(attempt_id, control_token, force?)`
- `claim_attempt_control(attempt_id, ttl_secs?, force?, claimed_by_client_id?)`
- `get_attempt_control(attempt_id)`
- `release_attempt_control(attempt_id, control_token)`

观测（Feed-first）：
- `tail_attempt_feed(attempt_id, limit?, cursor?, after_log_index?, wait_ms?)`
- `tail_session_messages({attempt_id|session_id}, limit?, cursor?)`
- `tail_project_activity(project_id, limit?, cursor?, after_event_id?)`
- `tail_task_activity(task_id, limit?, cursor?, after_event_id?)`

改动/产物（有 guardrails）：
- `get_attempt_changes(attempt_id, force?)`
- `get_attempt_patch(attempt_id, paths[], force?, max_bytes?)`
- `get_attempt_file(attempt_id, path, start?, max_bytes?)`

审批（可透传给用户交互）：
- `list_approvals(attempt_id, status?, limit?, cursor?)`
- `get_approval(approval_id)`
- `respond_approval(approval_id, execution_process_id, status, denial_reason?, responded_by_client_id?, request_id?)`

## MCP Inspector（UI/CLI）

`@modelcontextprotocol/inspector` 可作为“外部客户端视角”的验收工具，用来检查：
- `tools/list` 的 tools 数量/名称是否符合预期
- 每个 tool 的 `outputSchema` / `annotations` / `execution.taskSupport`

要求：Node.js `^22.7.5`。

### UI 模式（stdio）

先编译 MCP server：
```bash
just mcp-build
```

启动 Inspector（会运行一个本地 proxy + Web UI）：
```bash
TARGET_DIR="$(cargo metadata --format-version=1 --no-deps | node -e 'const fs=require(\"fs\");const data=fs.readFileSync(0,\"utf8\");process.stdout.write(JSON.parse(data).target_directory);')"
npx @modelcontextprotocol/inspector -- "$TARGET_DIR/release/mcp_task_server"
```

安全提示：
- 不要设置 `DANGEROUSLY_OMIT_AUTH=true`（会禁用 proxy auth，风险极高）
- 默认仅绑定 `localhost` 即可；不要随意用 `HOST=0.0.0.0` 暴露到不可信网络

### CLI 模式（脚本化 smoke test）

```bash
TARGET_DIR="$(cargo metadata --format-version=1 --no-deps | node -e 'const fs=require(\"fs\");const data=fs.readFileSync(0,\"utf8\");process.stdout.write(JSON.parse(data).target_directory);')"
npx @modelcontextprotocol/inspector --cli --method tools/list -- "$TARGET_DIR/release/mcp_task_server"
```

## 默认参数（建议）

- `tail_attempt_feed`: `limit=50`
- `tail_attempt_feed`（long-poll）: `wait_ms<=30000` 且必须配合 `after_log_index`
- `tail_session_messages`: `limit=20`
- `tail_project_activity`/`tail_task_activity`: `limit=50`
- `get_attempt_changes`: `force=false`
- `get_attempt_file`: `max_bytes=65536`
- `get_attempt_patch`: `max_bytes=204800`

## Top “Avoid” Mistakes

1) **`cursor` 与 `after_*` 互斥**：`tail_attempt_feed` / `tail_project_activity` / `tail_task_activity`  
2) **`tail_attempt_feed.wait_ms` 必须配合 `after_log_index`**（否则返回 `code=wait_ms_requires_after_log_index`）  
3) **同一个请求同时传 `attempt_id` 和 `session_id`**（会返回 `code=ambiguous_target`）  
4) **遇到 `code=blocked_guardrails` 不看 `hint`**：通常需要 `force=true`、缩小 `paths`、或降低 `max_bytes`  
5) **`respond_approval` 的 `execution_process_id` 不匹配**：必须与该 approval 绑定的 execution 一致

## 从零启动（典型链路）

1. `list_projects` → 选 `project_id`
2. `list_repos(project_id)` → 组装 `repos=[{repo_id,target_branch}]`
3. `list_executors` → 选 `executor`（必要时再选 `variant`）
4. `create_task(project_id, title, description?, request_id?)` → 得到 `task_id`
5. `start_attempt(task_id, executor, repos[], variant?, request_id?, prompt?)` → 得到 `attempt_id/session_id/execution_process_id/control_token`
6. 轮询 `tail_attempt_feed`：
   - 第一次不传 `after_log_index`，记录返回的 `next_after_log_index`
   - 后续传 `after_log_index=next_after_log_index` 只拿增量日志
   - 需要低频调用但低延迟体验时：传 `wait_ms`（仅支持 after 模式，且上限 30000ms）
   - 若 `pending_approvals` 非空：对每个 `approval_id` 做 `get_approval`（拿 `tool_name/tool_input`）→ 透传给用户 → `respond_approval`
7. 需要改动/产物时：`get_attempt_changes` → `get_attempt_patch` / `get_attempt_file`
8. 结束：`stop_attempt(attempt_id, control_token, force?)`

## Attempt 控制（Lease / `control_token`）

`send_follow_up` 与 `stop_attempt` 属于 **写操作**，必须提供有效的 `control_token`（从 `start_attempt` 或 `claim_attempt_control` 获取）。

常见用法：
- **初始控制**：`start_attempt` 返回 `control_token`（带 TTL，过期需续租）
- **续租/接管**：`claim_attempt_control(attempt_id, ttl_secs?, force?)`
  - `force=false` 且他人未过期持有时：返回 `code=attempt_claim_conflict`
  - `force=true`：直接抢占并返回新的 `control_token`
- **查看状态**：`get_attempt_control(attempt_id)`（owner + expires_at + expired）
- **释放**：`release_attempt_control(attempt_id, control_token)`

## `tail_attempt_feed`：两种模式（不要混用）

- **增量 tail（推荐）**：用 `after_log_index` 只拿新日志  
  示例：
  ```json
  {"attempt_id":"...","limit":50,"after_log_index":123}
  ```

- **增量 tail + long-poll（推荐给外部编排器）**：无新日志/审批时阻塞等待一小段时间  
  示例：
  ```json
  {"attempt_id":"...","limit":50,"after_log_index":123,"wait_ms":15000}
  ```

- **翻旧页**：用 `cursor` 拿更旧历史（返回 `page.next_cursor`）  
  示例：
  ```json
  {"attempt_id":"...","limit":50,"cursor":123}
  ```

## 人类接管 / Kanban 自动刷新（project/task 级别）

- `tail_project_activity(project_id, after_event_id?)`：刷新“项目发生了什么”
- `tail_task_activity(task_id, after_event_id?)`：刷新“某任务发生了什么”

当你只需要“最新变化”时优先使用 `after_event_id`；需要翻历史时使用 `cursor`。

## 错误与恢复（重要）

### 1) 参数错误：JSON-RPC error（客户端调用不合法）

当必填参数缺失、类型/格式不匹配（例如 UUID 解析失败）、或 tool 输入 schema 不满足时，服务器会返回 **JSON-RPC error**（通常 `code=-32602` / `invalid_params`）。  
这类错误一般不应盲目重试：应修正调用参数后再发起请求。

示例（伪）：
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": { "path": "attempt_id", "hint": "expected UUID" }
  }
}
```

### 2) 业务错误：tool structured_error（可恢复/可编排）

当请求语义合法但触发业务约束（例如混用分页、幂等冲突、guardrails 阻断等）时，服务器会返回 **tool-level error**：
- tools/call 的 JSON-RPC 请求本身成功
- 但 `result.isError=true`
- 且 `result.structuredContent` 为结构化错误对象：
  - `code`：稳定错误码
  - `retryable`：是否建议重试
  - `hint`：下一步建议（编排器可直接展示/执行）
  - `details`：结构化上下文（对象）

示例：diff 预览被 guardrails 阻断（`code=blocked_guardrails`）
```json
{
  "isError": true,
  "structuredContent": {
    "code": "blocked_guardrails",
    "retryable": false,
    "hint": "Patch blocked by diff preview guardrails. Retry with force=true to bypass.",
    "details": { "attempt_id": "...", "blocked_reason": "threshold_exceeded" }
  }
}
```

常见业务错误码（非穷尽）：
- `mixed_pagination`：同时传了 `cursor` 与 `after_*`
- `ambiguous_target`：同时传了 `attempt_id` 与 `session_id`
- `blocked_guardrails`：`get_attempt_changes/patch/file` 被 guardrails 阻断（提示通常会建议 `force=true` 或缩小范围）
- `idempotency_conflict`：同一个 `request_id` 被不同参数复用
- `idempotency_in_progress`：同一个 `request_id` 正在执行（`retryable=true`，按 hint 稍后重试）
- `wait_ms_requires_after_log_index`：`tail_attempt_feed` 使用 `wait_ms` 时必须提供 `after_log_index`
- `wait_ms_too_large`：`wait_ms` 超出服务器允许上限
- `attempt_claim_required`：写操作需要 lease，但当前无有效 lease（或已过期）
- `attempt_claim_conflict`：lease 被他人持有且未过期（通常是未提供 token 或抢占未 force）
- `invalid_control_token`：提供的 `control_token` 不匹配或已过期
