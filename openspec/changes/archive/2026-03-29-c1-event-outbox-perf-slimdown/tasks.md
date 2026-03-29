## 1. EventOutbox 最小写入 + 查询索引

- [x] 1.1 将 `EventOutbox::mark_published/mark_failed` 改为直接 `UPDATE`（无 `find_by_id` 读改写），并保持 RecordNotFound 语义（验证：`cargo test -p db outbox_enqueue_fetch_and_marking`）
- [x] 1.2 新增 DB migration：为 unpublished 查询增加更匹配的 outbox 复合索引（并在必要时移除冗余索引）（验证：`pnpm run prepare-db:check`）

## 2. EventService flush 热路径瘦身

- [x] 2.1 outbox worker idle 轮询改为自适应退避（backoff），backlog 存在时尽快 drain 并避免饥饿（验证：`cargo test -p events flush_pending_publishes_outbox_and_emits_patches`）
- [x] 2.2 `dispatch_entry` 为 task/project/workspace 增加 fast path：避免 payload clone + typed 反序列化，仅提取所需字段（验证：`cargo test -p events`）

## 3. 验收 / 归档 / 提交

- [x] 3.1 运行并修复直到通过：`just qa`、`just openspec-check`
- [x] 3.2 归档该 change（`openspec archive -y c1-event-outbox-perf-slimdown`）并创建最终 commit：`refactor: event-outbox-perf-slimdown`
