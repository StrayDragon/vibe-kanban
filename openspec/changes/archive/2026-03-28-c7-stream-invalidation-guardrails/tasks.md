## 1. Backend：hints 覆盖护栏

- [x] 1.1 在 `crates/logs-axum/src/lib.rs` 增加表驱动单测：覆盖 `/tasks/<id>`、`/workspaces/<id>`（含 `value.task_id`）、`/execution_processes/<id>` 的 add/replace/remove，以及 JSON Pointer `~0/~1` 解码与无关 patch 返回 None。
- [x] 1.2 在 `crates/app-runtime/src/lib.rs` 增加/扩展 SSE 单测：覆盖 execution_process 相关 patch 时 `event: invalidate` 且同 `seq` 不出现第二条事件。
- [x] 1.3 验收：`cargo test -p logs-axum -p app-runtime -p server` 通过。

## 2. Frontend：batcher 上限 + 调度优化

- [x] 2.1 在 `frontend/src/contexts/eventStreamInvalidationBatcher.ts` 引入 `MAX_UNIQUE_IDS_PER_BATCH=512`（或同等常量），超阈值时立即执行一次 `queryClient.invalidateQueries()` 并 reset/取消 pending flush。
- [x] 2.2 实现 flush 调度策略：页面可见优先 `requestAnimationFrame` flush；页面不可见或无 RAF 时 fallback `setTimeout(0)`，确保不会无限延迟。
- [x] 2.3 增加/更新前端单测：覆盖阈值降级只触发一次、reset 能取消 pending、不可见页 fallback 仍能 flush（`vi.useFakeTimers()`）。
- [x] 2.4 验收：`pnpm -C frontend test && pnpm -C frontend run lint` 通过。

## 3. Verification

- [x] 3.1 Run `just qa`.
- [x] 3.2 Run `just openspec-check`.
