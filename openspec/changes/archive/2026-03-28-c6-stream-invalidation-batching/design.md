## Context

当前前端通过 `/events` SSE stream 收到两类用于 cache 刷新的事件：
- `invalidate`：后端提供的 targeted hints（taskIds/workspaceIds/hasExecutionProcess）
- `json_patch`：完整 JSON Patch（前端再从 path/value 推导 invalidations 作为 fallback）

后端实现（`crates/app-runtime/src/lib.rs` 的 `stream_events`）在同一条 `seq` 的 `JsonPatch` 上，通常会先发 `invalidate` 再发 `json_patch`。前端（`frontend/src/contexts/EventStreamContext.tsx`）为避免重复工作，会用 `lastEventId` 做一次性去重，但：
- 后端仍然为同一条更新发送了两条 SSE event（网络与编码开销翻倍，且 `json_patch` 可能很大）。
- 前端在高频更新下仍会频繁调用 `queryClient.invalidateQueries`（同一 id / queryKey 的重复 invalidation 触发多次调度与潜在 rerender）。

## Goals / Non-Goals

**Goals:**
- `/events` 在 hints 可用时只发送一次 SSE event，避免冗余 `json_patch` payload。
- 前端对 invalidations 做批处理与去重，保证“按需最小 invalidation”。
- 保持 `seq`/resume 与 `invalidate_all` 语义不变，UI 行为不变。

**Non-Goals:**
- 不修改 WebSocket stream 协议与 payload shape。
- 不引入新的持久化或跨请求缓存；不修改 DB schema。
- 不在本变更中重构 React Query 的 query key 设计。

## Decisions

1) **后端：`invalidate` 覆盖时不再发送同 `seq` 的 `json_patch`**
- 方案：在 `stream_events` 里，对每条 `SequencedLogMsg`：
  - 若 `to_invalidate_sse_event()` 返回 `Some(event)`，仅发送该 `invalidate` event（id=seq）
  - 否则发送 `msg.to_sse_event()`（保持 `json_patch` 作为 fallback）
- 理由：在 hints 可用时，`json_patch` 对前端 invalidation 已无必要；保留 fallback 以覆盖 hints 为空的 patch。
- 备选：继续发送两条 event 并依赖前端去重（实现简单但浪费网络/CPU）。

2) **前端：引入 invalidation batcher（按 tick/frame 合并 + 去重）**
- 方案：在 EventStreamProvider 内创建一个 batcher，收集来自：
  - `invalidate` event 的 hints
  - `json_patch` event 推导出的 invalidations（fallback）
  然后在短窗口内合并并 flush，一次性触发最小集合的 `invalidateQueries`。
- 调度策略：使用 `setTimeout(0)`（或同等的 macrotask）来聚合同一 burst 的多条 SSE event，并保证不会无限延迟。
- 去重策略：用 `Set` 去重 taskIds/workspaceIds；对 `hasExecutionProcess` 用布尔 OR；flush 时只对唯一 id 生成 queryKey。
- 理由：减少重复 invalidation 调度；在 burst 更新下显著降低 CPU 与内存抖动。
- 备选：立即 invalidation（当前做法）；或更长窗口 batching（更省但延迟更大）。

3) **保持 `invalidate_all` 立即生效**
- 方案：收到 `invalidate_all` 时直接 `queryClient.invalidateQueries()`（不走 batcher），并允许 batcher 的 pending 在后续 flush 时自然落空或被重置。
- 理由：这是明确 resync-required 信号，应尽快把缓存标记为 stale。

## Risks / Trade-offs

- [Hints 覆盖不足导致漏刷新] → Mitigation：保留 `json_patch` fallback；通过单测锁定 hints 覆盖路径（workspace/task/exec-process）。
- [Batching 引入轻微延迟] → Mitigation：窗口限定为单 tick（`setTimeout(0)`），不会造成可感知延迟；必要时可改为 `queueMicrotask` 或 `requestAnimationFrame`。
- [极端 burst 下集合过大] → Mitigation：仍使用 targeted ids；如后续出现异常大集合，再引入硬上限并降级为 `invalidate_all`（本变更不做）。

## Migration Plan

- 无数据迁移；为纯运行时行为优化。
- 回滚策略：单个 commit 可 revert；对外 API/协议影响仅限 `/events` 的事件冗余减少（前端仍兼容 `json_patch` fallback）。
