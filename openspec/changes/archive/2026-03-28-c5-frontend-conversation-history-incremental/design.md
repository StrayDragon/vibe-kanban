## Context

execution processes 的 realtime 更新会驱动 attempt 详情页的 conversation/history 渲染。当前实现中，`frontend/src/hooks/execution-processes/useConversationHistory.ts` 在多个阶段会对全量 processes 做重复的：
- sort（按时间/索引）
- flatten（把每个 process 的 entries 合并为单一数组）
- count/分组/统计

并且这些派生结果会被多处消费，导致同一条 patch 触发多次全量 O(n log n) 或 O(n) 扫描。对长对话历史与高频 patch 来说，这会造成明显 CPU 峰值与内存抖动，并放大 rerender 范围。

## Goals / Non-Goals

**Goals:**
- conversation history 派生从“全量重建”收敛到“按 process id 局部更新”。
- 不影响 UI 可见语义：entries 顺序、加载/空态、以及与 processes 切换的行为保持一致。
- 让未受影响的子列表保持 referential equality，最大化 React memo 的收益。
- 为大数据量构造提供回归测试，避免未来回退到全量重算。

**Non-Goals:**
- 不重做日志/对话渲染组件体系（VirtualizedList / NormalizedConversation 的深层优化另起 change）。
- 不调整后端协议；仅消费既有 `seq`/`invalidate` hints 与现有数据结构。

## Decisions

1) **按 process id 的派生缓存（增量更新）**
- 方案：为每个 execution process 维护派生缓存（例如 `flattenedEntries`、`counts`、`lastSeenSeq`），当某个 process 的 entries 变化时，仅重算该 process 的派生结果，再与全局顺序合并。
- 备选：继续在 hook 内每次对全量 processes 做 sort/flatten（简单但代价高）。

2) **统一排序入口与稳定 tie-break**
- 方案：在 `ExecutionProcessesContext` 或单一 hook 中产出 canonical 的 `processesSorted` 与 `processesById`，其他 hooks 只消费该结果；排序规则明确 tie-break（例如 `(created_at, id)` 或 `(started_at, id)`），避免重复 sort 与不稳定顺序。
- 备选：多个 hooks 各自排序（导致重复计算与不一致风险）。

3) **缓存上限与低内存友好**
- 方案：对 per-process 派生缓存设置上限（例如 LRU，默认上限与现有逻辑一致或更低），并在超过上限时丢弃最久未访问的 process 派生数据，避免长会话导致常驻内存增长。
- 备选：无上限缓存（在多 attempt/多 process 切换下有内存风险）。

4) **测试以“行为一致 + 复杂度不回退”为目标**
- 方案：补充大输入下的回归测试：
  - 行为一致：顺序、分组、loading 状态与现有一致
  - 复杂度守护：在 N 个 process 与 M 条 entry 下，局部更新不应触发全量重算（用 spy/计数器或微基准式断言）
- 备选：只做轻量快照测试（容易漏性能回退）。

## Risks / Trade-offs

- [缓存一致性 bug] → Mitigation：缓存 key 使用 process id + 版本/seq；遇到无法安全增量的输入时明确 fallback 到一次全量重建，并加测试覆盖边界。
- [增加实现复杂度] → Mitigation：将缓存逻辑封装为单独模块/纯函数，保持 hooks 简洁；避免在组件树中分散状态。
- [轻微 UI 延迟] → Mitigation：仅在必要时批处理（microtask/rAF）；默认仍保持实时更新体验。

## Migration Plan

- 无需用户数据迁移；前端内部实现优化。
- 回滚策略：单 commit revert；不影响后端协议与 DB。
