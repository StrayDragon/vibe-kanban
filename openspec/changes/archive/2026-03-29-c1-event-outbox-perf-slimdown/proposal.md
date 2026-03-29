## Why

当前 realtime patch 的生成依赖 DB `event_outbox` + 后台 worker 的轮询 flush。现状存在两类不必要的 CPU/内存与 DB 压力放大：

- idle 状态仍按固定间隔（250ms）醒来查询 DB；
- flush 路径对每条 outbox 记录做 `find_by_id -> update` 的读改写，并且对多类事件做 payload clone + 反序列化。

在低功耗/小机器环境（或高频任务操作）下，这些开销会造成持续 CPU wakeup、更多 SQLite roundtrip、以及更高的内存分配抖动。

## What Changes

- outbox worker 的 idle 等待改为“自适应退避”（有 backlog 时尽快 drain；连续空轮询时逐步拉长 sleep），减少空转唤醒频率。
- `EventOutbox::mark_published/mark_failed` 改为最小写入：不再先 `SELECT` 再 `UPDATE`，改为基于主键的直接 `UPDATE`（并保持 RecordNotFound 语义）。
- `EventService::dispatch_entry` 对常见事件类型避免 payload clone + typed 反序列化：
  - task/project 事件直接使用 outbox 行的 `entity_uuid` 作为目标 id；
  - workspace 事件仅提取 `task_id` 字段；
  - execution_process / scratch 等仍保留 typed 反序列化（需要更多字段）。
- 为 `fetch_unpublished(published_at IS NULL ORDER BY created_at)` 增加更匹配的索引，减少 worker 查询成本。
- 增补测试覆盖：确保 flush 语义不变（成功标记 published；失败递增 attempts+last_error；patch 仍按预期发出）。

## Capabilities

### New Capabilities
- `event-outbox-publishing`: 定义 outbox 事件发布的可靠性与性能护栏（最小 DB 写入、idle 退避策略、查询索引约束）。

### Modified Capabilities
- （无）

## Impact

- Backend: `crates/events/src/lib.rs`（outbox worker flush/poll、dispatch 路径去 clone+反序列化）
- DB: `crates/db/src/models/event_outbox.rs`（mark_* 最小写入、查询）
- DB Migration: `crates/db/migration/src/*`（outbox 索引）
- Tests: `crates/events/src/lib.rs`、`crates/db/src/models/event_outbox.rs`

## Goals

- 在 outbox 空闲时显著降低 CPU wakeup 与 DB 读写次数。
- 在 backlog 存在时保持快速 drain（不牺牲实时性）。
- 减少 flush 热路径的内存分配与 JSON 反序列化开销。

## Non-goals

- 不改变 outbox 的业务语义（事件类型、payload schema、patch 内容与顺序）。
- 不引入新的外部队列/依赖（仍使用 SQLite outbox 表）。
- 不在本变更中重写 realtime stream 协议。

## Risks

- idle 退避可能引入“长时间空闲后首个事件”的额外延迟（通过限制最大退避上限并在 backlog 出现后立即 reset 缓解）。
- 索引调整会改变 SQLite 的 query plan（通过 migration + tests + `prepare-db:check` 校验）。

## Verification

- 单测：`cargo test -p db event_outbox`，`cargo test -p events flush_pending_publishes_outbox_and_emits_patches`
- 全量：`just qa`、`just openspec-check`
