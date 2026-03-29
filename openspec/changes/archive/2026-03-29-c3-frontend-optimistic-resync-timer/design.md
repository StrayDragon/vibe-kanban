## Context

`useAllTasks` 与 `useProjectTasks` 会维护 optimistic inserts/overrides/tombstones，并在 stream 状态与 optimistic patch 不一致时触发 `resync('optimistic-stale')`。为了避免 optimistic 覆盖长期遮蔽权威状态，当前实现使用一个 250ms 的 tick（只要存在 optimistic state 就持续 setTimeout 触发），以便周期性评估是否满足 resync 条件。

但 resync 条件本质由 meta 时间窗决定：

- `setAt + resyncAfterMs`
- `lastResyncAt + minResyncGapMs`
- `resyncAttempts < maxAttempts`

因此无需高频轮询即可在正确时间点触发检查。

## Goals / Non-Goals

**Goals:**

- 用 one-shot timer 取代固定 250ms tick，按需触发 optimistic-stale 检查。
- 保持现有候选筛选与 resync 门控逻辑不变（语义一致）。
- 提取纯函数用于“下一次检查时间”计算，并用 Vitest 覆盖边界。

**Non-Goals:**

- 不改变 optimistic store 的数据结构与写入时机。
- 不改变 websocket stream 的协议与后端 invalidation 行为。
- 不在本变更中做任务列表/看板虚拟化等更大范围前端优化。

## Decisions

### 1) 继续使用 tick state，但仅在“下一次可能满足条件”的时间点递增

**Decision:** 保留 `optimisticStaleTick` 作为触发 effect 的最小状态，但由 one-shot timer 在计算出的 `nextEligibleAt` 时间点递增，而不是每 250ms 递增。

**Why:** 该做法对现有代码侵入最小：不改变 resync effect 的主要逻辑与依赖关系，只替换“触发频率”。

**Alternative considered:** 将 resync 检查与 timer 调度合并为单个 effect（更少 state），但需要更复杂的闭包捕获/同步，且更容易引入 stale closure 风险。

### 2) 下一次调度时间只基于 meta 时间窗计算

**Decision:** `nextEligibleAt` 由所有 optimistic meta 的最小 `eligibleAt` 决定，不尝试预测 future 的 satisfied/unsatisfied 状态。

**Why:** satisfied/unsatisfied 依赖当前 stream 数据与 patch 内容，属于运行时状态；meta 时间窗是必要条件，足以减少大量无效 wakeup。

## Risks / Trade-offs

- [Risk] 调度时间计算错误导致 resync 触发异常 → **Mitigation**：提取纯函数并覆盖 attempts/gap/after 的边界用例。
- [Trade-off] 仍保留一个 tick state（但触发次数显著降低）→ **Mitigation**：后续如需要可进一步改为无 tick 的“内部循环”调度，但不在本变更范围内。

## Migration Plan

- 无需迁移；仅前端代码变更。

## Open Questions

- （无）
