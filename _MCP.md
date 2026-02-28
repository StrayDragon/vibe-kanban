# Vibe Kanban MCP（`mcp_task_server`）— Closed‑Loop Agent Guide

目标：让“会自己调用工具”的客户端（LLM agent / 编排器）用**尽量少的 MCP tools**把一次 attempt 跑完，并且能稳定闭环：**状态 → 日志 → 改动 → 产物 → 跟进/停止**。

如果只记一条链路：`get_attempt_status → tail_attempt_logs → get_attempt_changes`。

## 术语

- `attempt_id`：一次任务执行的 workspace id（MCP 里统一叫 attempt）。
- `session_id`：一次会话上下文（`follow_up` 把 prompt 发给它；也可以只给 `attempt_id` 让服务端 resolve 最新 session）。
- `execution_process_id`：一次具体执行进程（`tail_attempt_logs` 读取它的日志）。
- `state`：`idle | running | completed | failed`（attempt 的粗粒度状态；不包含 dev server 这种长寿进程）。

## 工具清单（实际可用）

发现/选择：
- `list_projects` / `list_repos(project_id)` / `list_executors`

任务/attempt：
- `list_tasks(project_id, status?, limit?)` / `get_task(task_id)` / `create_task(project_id, title, description?, request_id?)`
- `update_task(task_id, title?, description?, status?)` / `delete_task(task_id)`
- `list_task_attempts(task_id)`
- `start_task_attempt(task_id, executor, repos[], variant?, request_id?)`

闭环观测：
- `get_attempt_status(attempt_id)`
- `tail_session_messages({attempt_id|session_id}, cursor?, limit?)`（会话“转写”：每个 turn 的 prompt/summary）
- `tail_attempt_logs(attempt_id, channel?, limit?, cursor?, after_entry_index?)`（执行日志：默认 normalized）
- `get_attempt_changes(attempt_id, force?)`（summary + changed files；可能被 guard 挡住）

按需取产物（有 guardrails）：
- `get_attempt_patch(attempt_id, paths[], force?, max_bytes?)`
- `get_attempt_file(attempt_id, path, start?, max_bytes?)`

控制：
- `follow_up(action=send|queue|cancel, {attempt_id|session_id}, prompt?, variant?, request_id?)`
- `stop_attempt(attempt_id, force?)`

`get_context`：只有 MCP 进程能拿到本地 workspace context 时才会出现，外部编排器不要依赖它。

## 默认参数（建议）

- `tail_attempt_logs`: `channel=normalized`, `limit=50`
- `tail_session_messages`: `limit=20`
- `get_attempt_changes`: `force=false`
- `get_attempt_file`: `max_bytes=65536`
- `get_attempt_patch`: `max_bytes=204800`

## Top 3 “Avoid” Mistakes

1) **同一个请求同时传 `attempt_id` 和 `session_id`**（会返回 `code=ambiguous_target`）  
2) **`tail_attempt_logs` 混用 `cursor` 和 `after_entry_index`**（会返回 `code=mixed_pagination`）  
3) **把 `follow_up(action=cancel)` 当成“停止进程”**：它只取消队列；要停正在跑的执行用 `stop_attempt`。同时 `follow_up(action=send|queue)` 必须带 `prompt`。

## 从零启动（典型链路）

1. `list_projects` → 选 `project_id`
2. `list_repos(project_id)` → 选 `repo_id + target_branch`
3. `list_executors` → 选 `executor`（必要时再选 `variant`）
4. `create_task(project_id, title, description?, request_id?)` → 得到 `task_id`
5. `start_task_attempt(task_id, executor, repos[], variant?, request_id?)` → 得到 `attempt_id`
6. `follow_up({"action":"send","attempt_id":..., "prompt":...})`

如果 `follow_up` 报 `code=no_session_yet`：先 `get_attempt_status(attempt_id)`，等 `latest_session_id` 非空再重试。

## 闭环：状态 → 日志 → 改动 → 产物

### 1) `get_attempt_status`

看这些字段：
- `state`
- `latest_session_id`（用于 `tail_session_messages` / `follow_up`）
- `latest_execution_process_id`（用于理解日志来源；`tail_attempt_logs` 内部会自动 resolve）
- `failure_summary`（只作提示，细节看日志）

### 2) `tail_attempt_logs`：两种模式（不要混用）

- **增量 tail（推荐）**：用 `after_entry_index` 只拿新日志
  - 第一次：`after_entry_index` 省略 → 先拿最近一页，记录 `max(entry_index)`
  - 后续轮询：`after_entry_index = last_seen_entry_index`

示例（增量）：
```json
{"attempt_id":"...","limit":50,"after_entry_index":123}
```

- **翻旧页**：用 `cursor` 拿更旧历史（`page.next_cursor` 回传）

示例（翻历史）：
```json
{"attempt_id":"...","limit":50,"cursor":123}
```

### 3) `get_attempt_changes`：先不 force

- 先 `force=false` 拿 `summary` 做判断
- 需要文件列表再 `force=true`
- 当 `blocked=true` 时，响应会带 `code/hint` 指向下一步（通常是“用 force=true 重试”）

### 4) `get_attempt_patch` / `get_attempt_file`：按需取内容（有上限）

这些工具会返回：
- `blocked` / `blocked_reason`（例如 `path_outside_workspace | size_exceeded | too_many_paths | threshold_exceeded`）
- `truncated=true`（当返回内容被 `max_bytes` 截断）

示例（patch，强制绕过 diff guard，且只取关键文件）：
```json
{"attempt_id":"...","paths":["my-repo/src/lib.rs"],"force":true,"max_bytes":200000}
```

示例（file，读一小段）：
```json
{"attempt_id":"...","path":"my-repo/src/main.rs","start":0,"max_bytes":65536}
```

## 错误与恢复（重要）

多数“可恢复”错误会以 JSON 形式返回，并包含：
- `code`（稳定字符串，例如 `no_session_yet | ambiguous_target | mixed_pagination | blocked_guardrails`）
- `retryable`（是否可以不改参数直接重试）
- `hint`（下一步建议，通常点名下一个 tool）

遇到 `blocked=true` 时也优先看 `hint`（例如“缩小 paths / 降低 max_bytes / 用 force=true 重试”）。

