## Why

当前前端在部分“只读 + 高频 patch”的页面仍会对每次 realtime 更新执行全量派生：`Object.values + sort + group`、重复 `Date.parse/new Date()`、以及重复构造新数组/对象。这会在 tasks 数量较大或更新 burst 时显著放大 CPU 与短期内存分配，导致 UI 卡顿、频繁 GC，并降低整体吞吐。

## What Changes

- 前端：将 archived kanban tasks（`useArchivedKanbanTasks`）从“每次 patch 全量排序/分桶”改为基于 `taskDerivation` 的增量派生缓存（复用 `useAllTasks`/`useProjectTasks` 的模式），并利用 WS `invalidate.taskIds` 只对变更 id 做更新。
- 前端：减少 derived 结构的重复分配与重复时间戳解析（缓存 `created_at` 的 ms 值；仅在必要时重建数组）。
- 测试：补齐 `taskDerivation`/archived tasks 派生的单测，锁定排序与按 status 分桶语义，并约束“未受影响列表保持引用稳定”的性能契约。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `frontend-performance-guardrails`: 增加对“realtime 衍生结构必须增量更新、并尽可能保持引用稳定”的约束，避免单条 patch 触发全量 rebuild。

## Impact

- Frontend: `frontend/src/hooks/archived-kanbans/useArchivedKanbanTasks.ts`, `frontend/src/hooks/tasks/taskDerivation.ts`（复用/补测）
- 风险：增量派生的边界条件（插入/删除/跨 status 移动）若处理不当会导致排序或分桶不一致；需要测试覆盖与回退到 full rebuild 的兜底逻辑
- Verification: `pnpm -C frontend run check`, `pnpm -C frontend run lint`, `pnpm -C frontend run test`, `just qa`, `just openspec-check`

