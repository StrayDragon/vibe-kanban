# MCP Agent Access Control（Attempt Lease）变更对比与下线评估

本文件对比 `mcp-agent-access-control` 变更 **前/后** 的 MCP 行为差异，并评估“旧版 MCP”（不带 attempt lease 的行为）是否可以下线。

## 范围与术语

- **范围**：`mcp_task_server`（MCP control plane）在 attempt 级别的访问控制、观测长轮询、以及 approvals 审计字段默认值。
- **attempt lease / control_token**：attempt 写操作的“控制权租约”（bearer token）。用于协调外部编排器与人类接管（UI）并发场景。

## Before → After：行为变化一览

### 1) Attempt 观测：`tail_attempt_feed` 增加 long-poll

**Before**
- 仅支持“立即返回”的 poll；外部编排器为了低延迟只能提高轮询频率，带来额外 CPU/IO 压力。

**After**
- `tail_attempt_feed` 新增 `wait_ms`（上限 30000）。
- **仅允许**在提供 `after_log_index` 的增量模式下使用 `wait_ms`：
  - 若传 `wait_ms` 但未传 `after_log_index`：返回 `code=wait_ms_requires_after_log_index`
  - 若 `wait_ms` 超上限：返回 `code=wait_ms_too_large`

### 2) Attempt 写操作：新增 lease，写操作强制 `control_token`

**Before**
- `send_follow_up` / `stop_attempt` 不具备统一的“控制权”概念；多 client/人类接管并发时容易互相踩踏（例如：外部编排器与 UI 同时 follow-up / stop）。

**After**
- `start_attempt` 响应新增：
  - `control_token`
  - `control_expires_at`
- 新增 MCP tools：
  - `claim_attempt_control`（获取/抢占控制权）
  - `get_attempt_control`（查询当前 owner + expires_at + 是否过期）
  - `release_attempt_control`（释放控制权）
- 写操作强制校验 token：
  - `send_follow_up` / `stop_attempt` 入参新增 `control_token`，并在后端校验
  - 典型错误码：
    - `attempt_claim_required`：当前没有有效 lease
    - `attempt_claim_conflict`：lease 被其他 client 持有且未过期（返回 owner/expires_at 提示）
    - `invalid_control_token`：token 不匹配或已过期

### 3) 审批审计：`respond_approval` 默认填充 `responded_by_client_id`

**Before**
- 外部编排器“弹窗批准”场景下，若客户端未显式传 `responded_by_client_id`，服务端可能无法稳定记录响应方身份。

**After**
- `respond_approval` 若未提供 `responded_by_client_id`，服务端会从 MCP peer info 派生默认值并持久化（无 peer 时使用 `mcp:unknown`）。

## 典型客户端升级步骤（外部编排器/OpenClaw 类）

1. `start_attempt(...)` 后保存 `control_token` 与 `control_expires_at`。
2. 所有写操作都带上 `control_token`：
   - `send_follow_up(..., control_token, ...)`
   - `stop_attempt(attempt_id, control_token, ...)`
3. 需要接管/续租时使用：
   - `claim_attempt_control(attempt_id, ttl_secs?, force?)` → 新 `control_token`
4. 观测侧建议改为：
   - `tail_attempt_feed(after_log_index=K, wait_ms=10000~30000)` 做低频调用 + 低延迟。

## “旧版 MCP”是否可下线？

### 结论
- **可以下线旧行为**（不带 lease 的写操作方式），并以“必须持有 attempt 控制权”作为唯一写入路径。

### 理由（与 Kanban 多 agent 并发目标一致）
- **并发安全**：lease 让“外部编排器/多个 agent/UI 人类接管”在 attempt 级别形成明确的互斥与抢占语义，避免写入踩踏。
- **可恢复性**：lease 有 TTL，可过期回收；外部编排器崩溃后不会永久占用控制权。
- **观测成本更低**：long-poll 降低高频轮询带来的资源开销，同时保持低延迟体验。

### 注意事项（下线执行层面）
- 本次变更已使写操作在无 token 时返回结构化错误；旧客户端将无法继续写入。
- 若仍需要“兼容旧客户端一段时间”，应在外部编排器层做适配（从 `start_attempt` 获取 token 并透传），而不是在服务端继续维持无 lease 的写入口。

## rmcp 升级前置评估（为下一个提案准备）

- 当前依赖：`crates/server/Cargo.toml` 使用 `rmcp = 0.12.0`（features: `server/client/transport-io/elicitation`）。
- 建议在升级 rmcp 前先做两件事：
  1. 固化“工具返回结构化内容 + 稳定错误码”的契约（本次已补齐 attempt lease + long-poll 相关错误码）。
  2. 以编排器视角补充回归：`start_attempt → long-poll feed → approvals → follow-up/stop` 全链路对拍（可沿用现有 server 侧单测结构）。

