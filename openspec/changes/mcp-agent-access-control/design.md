## Context

外部编排器（OpenClaw 类）希望通过 MCP 代理用户完成“创建任务 → 启动 attempt → 观察日志/活动 → 处理 approvals → 拉取 diff/patch → 停止/继续”的闭环。当前 MCP 已经具备这些工具，但在真实编排中会遇到两个突出问题：

1) **高频 tail 带来的性能与延迟抖动**  
编排器为了获得接近实时体验，通常会以很高频率轮询 `tail_attempt_feed(after_log_index)` 来拉取新日志。这在并发 attempt 数量上升时会带来不必要的 QPS 与 DB/CPU 开销。

2) **多方并发驱动的竞态（自动编排 vs 人类接管）**  
同一 attempt 可能同时被多个“上层代理”驱动（编排器自动 follow-up、人工临时插入 follow-up、脚本 stop），容易出现指令覆盖、顺序不可控、以及“谁在控制”的不透明。

本设计通过两项机制降低上述摩擦：
- `tail_attempt_feed` 增加 **可选长轮询**（`wait_ms`），在无新内容时阻塞等待一小段时间，显著降低轮询频率。
- 为 attempt 引入 **控制租约（lease）**（`control_token`），让“写操作”具备显式的所有权/接管语义。

## Goals / Non-Goals

**Goals:**
- 在不引入 MCP 持久在线推（notifications/streaming）的前提下，为外部编排器提供低频调用但低延迟的观测手段（long-poll）。
- 为 attempt 写操作（follow-up/stop）提供可编排的互斥控制语义（lease），支持自动化与人类接管之间的显式交接。
- 所有失败以结构化 tool error 表达，具备稳定 `code` 与可操作 `hint`。

**Non-Goals:**
- 不在本变更中升级 `rmcp` 或切换 MCP protocol version（作为后续独立提案）。
- 不在本变更中将 lease 强制扩展到 HTTP/SSE/WS（UI）路径；优先完成 MCP 控制面闭环。
- 不实现“通用流式订阅”（server push）；仍以 request/response + 有界等待为主。

## Decisions

### Decision 1: `tail_attempt_feed(wait_ms)` 采用“after 模式可选长轮询”

- **接口**：为 `tail_attempt_feed` 增加可选参数 `wait_ms`（毫秒）。  
- **约束**：仅当 `after_log_index` 提供时允许使用 `wait_ms`；与 `cursor`/older paging 互斥。  
- **语义**：当本次调用没有新日志条目且没有待处理 approvals 时，服务器最多等待 `wait_ms`：  
  - 一旦出现新 normalized log entry（`entry_index > after_log_index`）或出现新的 pending approval，则提前返回；  
  - 超时则返回空增量（`entries=[]`），并保持 `next_after_log_index` 不变。  
- **限额**：`wait_ms` 设定上限（例如 30_000ms），超过上限返回结构化错误 `code=wait_ms_too_large`。  

**实现选择：**
- 以 `MsgStore` 的 broadcast 通道作为“新日志到达”的低成本信号源；在没有 MsgStore（例如重启后）时降级为 `sleep(wait_ms)` + 重新查询一次（避免 tight loop）。
- approvals 变化以 `Approvals::subscribe_created()` 作为信号源（仅需要“有新 approval 出现”即可提前返回）。

### Decision 2: Attempt 写操作引入 DB-backed lease（`control_token`）

- **数据模型**：新增表 `attempt_control_leases`（attempt_id 一行），核心字段：
  - `attempt_id`（workspace uuid）
  - `control_token`（uuid，作为写操作 bearer token）
  - `claimed_by_client_id`（string，来源于 MCP peer info 或显式参数）
  - `expires_at`（timestamp，TTL）
  - `created_at/updated_at`
- **默认路径**：`start_attempt` 成功后自动创建 lease 并返回 `control_token`（带默认 TTL）。  
- **工具集**：
  - `claim_attempt_control`：创建/续租/（可选强制）抢占 lease，返回新的 `control_token` 与 `expires_at`。
  - `get_attempt_control`：查询当前 lease 状态（owner、是否过期、expires_at）。
  - `release_attempt_control`：校验 token 后释放 lease。
- **强制校验**：`send_follow_up` 与 `stop_attempt` 必须携带有效 `control_token`；否则返回结构化错误：
  - `attempt_claim_required`（无有效 lease）
  - `attempt_claim_conflict`（lease 被他人持有且未过期）
  - `invalid_control_token`（token 不匹配/已过期）

**原因与替代方案：**
- 替代方案 A：仅使用 `responded_by_client_id` 做审计，不做互斥 → 无法解决并发驱动竞态。
- 替代方案 B：仅内存锁 → 重启后丢失、且无法表达“接管”语义。
- DB-backed lease + TTL 提供了可恢复、可抢占、可编排的控制边界。

### Decision 3: `responded_by_client_id` 默认派生

当 `respond_approval.responded_by_client_id` 未提供时，服务器从 MCP peer info 派生默认值（例如 `mcp:<client_name>@<version>`），保证审计字段一致可用。

## Risks / Trade-offs

- **[资源占用]** 长轮询会占用连接与任务：  
  → 限制 `wait_ms` 上限；仅在 `after_log_index` 模式启用；超时立即返回。
- **[假锁]** 客户端崩溃可能遗留 lease：  
  → 使用 TTL；提供强制 `claim_attempt_control(force=true)` 抢占；返回明确冲突错误码供编排器决策。
- **[范围]** UI/HTTP 不受 lease 约束可能仍有竞态：  
  → 明确为后续提案；本次先把 MCP 编排闭环做稳。

## Migration Plan

1) 运行 DB migration（新增 `attempt_control_leases`）。  
2) 外部编排器更新：保存 `start_attempt.control_token`，后续 `send_follow_up/stop_attempt` 携带；采用 `tail_attempt_feed(wait_ms)` 代替高频轮询。  
3) 如出现抢占需求：通过 `get_attempt_control` 展示当前 owner 与到期时间，必要时用 `claim_attempt_control(force=true)` 接管。  

## Open Questions

- 是否需要在后续将 lease 统一下沉到 services 层，让 HTTP/UI 路径也共享同一互斥语义？  
- `rmcp` 升级到较新版本后，是否可以利用更完善的 schema/elicitation/工具元信息能力进一步简化编排器实现？  

