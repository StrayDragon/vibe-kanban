## Why

当前 `/events` SSE stream 在同一条 `seq` 的 realtime 更新上，经常同时发送：
- `invalidate`（后端提示的 targeted hints）
- `json_patch`（完整 JSON Patch）

前端为了兼容两种事件，会在每条 event 上触发多次 `queryClient.invalidateQueries`，在高频 patch/多 workspace 场景下放大 CPU 与内存开销（大量重复 invalidation、重复排队、重复 rerender 触发）。

## What Changes

- 后端：当某条 `JsonPatch` 可生成 `invalidate` hints 时，`/events` 仅发送 `invalidate`（不再为同一 `seq` 再发送冗余的 `json_patch`）。当无法生成 hints 时，仍发送 `json_patch` 作为 fallback。
- 前端：对来自 SSE 的 invalidation hints 做批处理与去重（按 event loop tick / frame 合并），以“最小 invalidate 次数”刷新相关 query key。
- 测试：补齐后端 `/events` 输出形状回归（同 `seq` 不重复发两个事件）与前端 invalidation batching 的单测，确保行为一致且性能更稳。

## Goals

- 显著降低高频更新时的 `invalidateQueries` 调用次数与重复工作，减少 CPU 与内存抖动。
- 保持 SSE 语义不变：仍以 `seq` 作为 resume/continuity 的基准，`invalidate_all` 仍作为 resync-required 信号。
- 保持 UI 可见行为不变：相关列表/状态仍能在 realtime 更新后及时刷新。

## Non-goals

- 不改变 WebSocket streams 协议（`seq`/`invalidate` 字段形状保持不变）。
- 不引入新的缓存层或持久化；不修改 DB schema。
- 不在本变更中重写前端 query key 体系或大规模重构 React Query 结构。

## Risks

- [SSE 事件类型变化] `invalidate` 覆盖范围不足可能导致漏刷新 → Mitigation：保留 `json_patch` fallback；为 hints 覆盖范围添加回归测试。
- [批处理延迟] batching 可能引入轻微刷新延迟 → Mitigation：限定在单 tick / 单 frame 内合并，保持交互可感知更新的及时性。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `refresh-task-attempts`: SSE invalidation 允许优先使用 `invalidate` hints，并要求对 invalidations 做去重/批处理以避免每条事件触发多次重复 invalidation。

## Impact

- Backend: `crates/app-runtime/src/lib.rs`（`stream_events` 输出策略）
- Frontend: `frontend/src/contexts/EventStreamContext.tsx`、`frontend/src/contexts/eventStreamInvalidation.ts`
- Verification: `cargo test -p app-runtime -p server`, `pnpm -C frontend test`, `just qa`, `just openspec-check`

## Verification

- Rust: 新增/更新单测覆盖 `/events` 在 hints 存在时只发一个 SSE event，并保持 `invalidate_all` 行为不变。
- Frontend: 新增单测覆盖 invalidation batching（多条 hints 合并后产生最小 `invalidateQueries` 调用集合）。
