## Context

realtime patch 的生成目前通过 DB `event_outbox` 解耦：各类模型写入时 enqueue outbox；`EventService` 在后台 loop 中周期性 `fetch_unpublished -> dispatch -> mark_published/mark_failed`，再将 patch 推入 `MsgStore`。

现状问题（偏性能与资源消耗）：

- outbox worker 空闲时仍按固定间隔轮询 DB（默认 250ms），在低功耗环境造成持续 wakeup；
- `mark_published/mark_failed` 采用 `SELECT by id -> UPDATE` 的读改写，额外增加 SQLite roundtrip；
- dispatch 路径对常见事件做 payload clone + typed 反序列化（`serde_json::from_value(entry.payload.clone())`），在 backlog 或高频写入时放大 CPU/alloc。

本变更以“不改变业务语义”为约束，主要通过最小写入 + 更少唤醒 + 更少反序列化来降低 CPU/内存占用。

## Goals / Non-Goals

**Goals:**

- outbox idle 时显著降低轮询频率与 DB 访问次数；backlog 存在时保持快速 drain。
- 去掉 per-row read-modify-write：`mark_*` 使用单条 UPDATE 并保持 RecordNotFound 语义。
- dispatch 对不需要 payload 的事件类型走 fast path，减少 clone/反序列化与临时分配。
- 为 worker 的主要查询提供更合适的索引，降低 `fetch_unpublished` 成本。

**Non-Goals:**

- 不改变 outbox 表结构、不改变 event_type / payload schema、不改变 patch 内容与顺序。
- 不引入新的外部队列/依赖（仍以 SQLite outbox 表为唯一来源）。
- 不在本变更中改造 WS/SSE 协议与客户端处理逻辑。

## Decisions

### 1) idle 轮询策略：自适应退避（backoff）而非 notify 驱动

**Decision:** 在 outbox 没有待发布事件时，sleep 间隔随连续空轮询次数增长并 capped；一旦处理到 backlog，立即 reset 到最小间隔。

**Why:** notify/唤醒需要把 `EventService` 贯穿到 DB 写入路径（或增加跨层全局 hook），会引入更大架构耦合。本变更聚焦最小侵入与确定性收益。

**Alternative considered:** 引入 `tokio::sync::Notify` 并在每次 enqueue 后触发唤醒（需要跨 crate/handler 注入与线程安全生命周期管理）。

### 2) `mark_published/mark_failed`：直接 UPDATE + rows_affected 校验

**Decision:** 使用 SeaORM `update_many().filter(id=..).col_expr(...)` 直接更新；当 `rows_affected == 0` 时返回 `DbErr::RecordNotFound`，保持原语义。

**Why:** 去掉读改写的额外 SELECT；对 SQLite 能明显减少 roundtrip 与锁竞争概率。

### 3) dispatch payload 处理：按 event_type 做最小字段提取

**Decision:** 对 task/project 事件使用 outbox 行的 `entity_uuid` 作为目标 id；对 workspace 事件从 payload 中只提取 `task_id`；对 execution_process / scratch 仍保留 typed 反序列化。

**Why:** task/project 的 patch 生成仅需要目标 id（随后会用 DB hydrate 获取完整实体），typed payload 在这些分支没有额外价值；workspace 分支只需要 `task_id`，不必反序列化整个结构。

**Alternative considered:** 统一保留 typed payload（简单但 CPU/alloc 放大）；或改 outbox 表结构增加结构化列（风险更高、迁移更重）。

### 4) 索引：为 unpublished 查询提供复合索引

**Decision:** 新增 `event_outbox(published_at, created_at)`（必要时再加 `id` 作为稳定 tie-break），用于 `published_at IS NULL ORDER BY created_at`。

**Why:** 现有单列 `published_at` 索引对排序不友好；复合索引能减少 worker 扫描与排序成本。

## Risks / Trade-offs

- [Risk] 退避导致长时间空闲后首个事件的 publish 延迟上升 → **Mitigation**：设置合理的 max backoff（例如 1s~2s），且 backlog 出现后立即 reset；同时保留快速 drain。
- [Risk] 使用最小字段提取会在 payload 不符合预期时更晚暴露问题 → **Mitigation**：保留 execution_process/scratch 的 typed 解析；workspace/task/project fast path 只依赖 outbox 行字段或单字段提取，并通过单测覆盖。
- [Risk] 新索引会增加写入开销与 DB 文件大小 → **Mitigation**：索引仅覆盖 outbox 关键查询；对比收益显著（降低频繁轮询查询成本）。

## Migration Plan

1. 新增 DB migration：创建 outbox 复合索引（必要时移除冗余索引）。
2. 运行 `pnpm run prepare-db:check` 以确保迁移可重放。
3. 运行相关单测 + `just qa` + `just openspec-check`。

## Open Questions

- （无）
