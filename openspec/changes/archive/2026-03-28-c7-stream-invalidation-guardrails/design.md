## Context

`/api/events` SSE 是前端 React Query cache 刷新与实时 UI 更新的关键通道。`c6-stream-invalidation-batching` 已将“同一条 `seq` 的冗余 `json_patch`”删除，并在前端对 invalidations 做了 batching + 去重，从性能角度是正确方向，但也引入了新的“正确性依赖”：

- 当后端认为 hints 可用时，前端不再有 `json_patch` 兜底，因此 **hints 推导必须完整且稳定**。
- 当 burst 高频更新发生时，前端 batching 需要在正确性不变的前提下 **限制峰值成本**（CPU/内存）。

本变更以“护栏”为主：把关键语义写入 spec，并用测试锁定最关键的回归面。

## Goals / Non-Goals

**Goals:**
- 通过 spec + tests 锁定 SSE `/api/events` 的关键行为（每个 `seq` 的事件输出策略、`invalidate_all` watermark 语义）。
- 提升 hints 推导的回归覆盖：更多 patch 形态、JSON Pointer 编码、边界输入。
- 为前端 invalidation batcher 增加上限与可控降级，避免极端 burst 造成不可接受的峰值 CPU/内存。
- 在不改变对外协议的前提下，允许优化 flush 调度以贴合渲染帧，降低不必要的 UI 抖动。

**Non-Goals:**
- 不新增或修改 SSE/WS payload schema 字段（不引入兼容层）。
- 不引入跨请求/跨会话的持久化缓存；不修改 DB schema。
- 不重构 React Query query key 体系，也不做大规模前端架构调整。

## Decisions

1) **Spec：在 `realtime-stream-resilience` 中补齐 SSE `/api/events` 语义**
- 增加明确要求：对于由 `SequencedLogMsg(JsonPatch)` 产生的更新，SSE MUST 对单个 `seq` 至多发出一个事件：
  - hints 可用 → `event: invalidate`（`id=seq`）
  - hints 不可用 → `event: json_patch`（`id=seq`）
- 增加明确要求：当 `resume_unavailable` 或 `lagged` 时，SSE MUST 发送 `event: invalidate_all`，且 `id=watermark`（payload 含 `watermark` 等字段）。
- 理由：把已实现的关键行为固化为 contract，避免未来“优化/重构”时无意破坏。

2) **Backend：以“推导函数行为”为中心补齐 hints 覆盖单测**
- 主要测试对象：`crates/logs-axum/src/lib.rs` 的 hints 推导逻辑（通过 `SequencedLogMsg::to_invalidate_sse_event()` 触发）。
- 覆盖面（表驱动即可）：
  - `/tasks/<id>`：add/replace/remove
  - `/workspaces/<id>`：add/replace/remove + `value.task_id` 注入 taskIds
  - `/execution_processes/<id>`：任意 op 触发 `hasExecutionProcess=true`
  - JSON Pointer segment decode：`~0`/`~1` 解码
  - 空/无关 patch 返回 None
- 理由：这是“hints 可用时不再发送 json_patch”的硬依赖；测试应尽量贴近推导函数本身而不是上层服务。

3) **Frontend：batcher 追加上限与降级（正确性优先，性能可控）**
- 在 `createInvalidationBatcher` 内引入常量阈值（例如 `MAX_UNIQUE_IDS_PER_BATCH`），以 `taskIds.size + workspaceIds.size` 作为近似压力指标。
- 当累计 unique ids 超阈值时：
  - 立即执行一次 `queryClient.invalidateQueries()`（等价于 `invalidate_all` 风格）
  - 清空 batch 并取消 pending flush（避免随后又重复触发大量 targeted invalidations）
- 理由：在极端 burst 下，全量 invalidation 虽更粗，但比 N×targeted invalidations 更可控；并且降级路径只在异常情况下触发。

4) **Frontend：flush 调度优先贴帧，后台不饿死**
- 调度策略：
  - 页面可见时优先 `requestAnimationFrame` flush（减少同一帧内重复 invalidation 调度造成的抖动）
  - 页面不可见（或无 RAF）时 fallback `setTimeout(0)`，保证后台/隐藏页也能及时 flush
- 理由：对“体验/性能”双优化；并且保持窗口很短（不会引入可感知延迟）。

## Risks / Trade-offs

- [降级更粗导致短时 refetch 增加] → Mitigation：阈值只作为异常保护；并用单测锁定降级只触发一次且会 reset batch。
- [RAF 在不可见页可能暂停] → Mitigation：不可见页用 `setTimeout(0)`；并在单测覆盖不会无限延迟。
- [阈值选择不合适] → Mitigation：先选保守阈值（只在异常 burst 触发），必要时通过 profiling 再调整并保持测试固定阈值。

## Migration Plan

- 无数据迁移；纯运行时护栏与测试增强。
- 回滚策略：单个 commit 可 revert。

