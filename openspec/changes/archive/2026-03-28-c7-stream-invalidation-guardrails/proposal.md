## Why

在 `c6-stream-invalidation-batching` 之后，`/api/events` SSE 在 hints 可用时只发送 `invalidate`，前端也会把 invalidations 做 batching + 去重。整体性能更好，但仍有三个稳定性/性能回归风险需要用“护栏”锁死：

- **Hints 覆盖回归风险**：`crates/logs-axum/src/lib.rs` 的 hints 推导若漏掉某些标识符，后端仍会认为 hints 可用并不再发送同 `seq` 的 `json_patch`，从而可能导致 UI 漏刷新。
- **Batcher 调度策略风险**：当前 flush 基于 `setTimeout(0)`，在前台高负载/渲染压力下可能出现不够贴帧的调度（体验/CPU 抖动可被放大）。
- **极端 burst 失控风险**：在异常 burst 下，`taskIds`/`workspaceIds` Set 可能短时间累积过大，单次 flush 仍会触发大量 `invalidateQueries`，带来 CPU/内存峰值。

## What Changes

- **Spec（contract）补齐**：
  - 追加/澄清 `/api/events` SSE 的“每个 `seq` 至多一个事件”语义：hints 可用时只发 `invalidate`；hints 不可用时只发 `json_patch`。
  - 锁定 `invalidate_all` 在 `resume_unavailable`/`lagged` 场景的关键字段（`id=watermark` + payload 字段）。
  - 为前端 invalidation batching 增加“上限与降级策略”要求（超过阈值时降级为 `invalidate_all` 风格的一次性 invalidation）。
- **Backend tests 加固**：补充覆盖更多 patch 形态/路径编码的 hints 推导行为（tasks/workspaces/execution_processes，add/replace/remove，JSON Pointer segment decode），确保 hints 推导完整且可回归。
- **Frontend batcher 加固**：
  - 为 batch 增加最大 unique id 数阈值；超过阈值时立即降级为一次性 `queryClient.invalidateQueries()` 并清空 batch。
  - 允许将 flush 调度策略改为更贴近渲染帧（例如优先 `requestAnimationFrame`，后台/不可见页 fallback 到 `setTimeout(0)`），降低不必要的 CPU 抖动。
- **Frontend tests 加固**：新增单测锁定阈值降级行为与调度策略（不会无限延迟/不会重复触发降级）。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `realtime-stream-resilience`: 明确 `/api/events` SSE 的单 `seq` 事件输出策略与 `invalidate_all` watermark 语义。
- `refresh-task-attempts`: 增加 invalidation batching 的“上限 + 降级”要求以防极端 burst。

## Impact

- Backend: `crates/logs-axum/src/lib.rs`, `crates/app-runtime/src/lib.rs`（tests / behavior guardrails）
- Frontend: `frontend/src/contexts/EventStreamContext.tsx`, `frontend/src/contexts/eventStreamInvalidationBatcher.ts`
- Specs: `openspec/specs/realtime-stream-resilience/spec.md`, `openspec/specs/refresh-task-attempts/spec.md`

## Goals

- 防止 hints 推导/事件输出策略出现 silent regression，保证 UI 不会因漏刷新而变“偶现不更新”。
- 在 burst/异常输入下限制 invalidation 的 CPU/内存峰值（可控降级，不崩不抖）。

## Non-goals

- 不更改现有 SSE/WS payload 的 schema（不新增字段、不引入兼容层）。
- 不在本变更内重构 React Query 的 query key 体系或引入新的 cache 层。

## Risks

- [降级导致 invalidation 更粗] 超阈值时会 `invalidateQueries()` 全量标记 stale，可能带来短时额外 refetch → Mitigation：阈值设置为“异常保护”，默认路径仍走 targeted invalidations，并通过单测锁定只在极端情况下触发。
- [调度策略差异] 引入 RAF 可能影响不可见页表现 → Mitigation：不可见页仍 fallback `setTimeout(0)`，并用单测/验收试验覆盖。

## Verification

- `cargo test -p logs-axum -p app-runtime`
- `pnpm -C frontend test && pnpm -C frontend run lint`
- `just qa && just openspec-check`

