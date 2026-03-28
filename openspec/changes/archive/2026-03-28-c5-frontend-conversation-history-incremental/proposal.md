## Why

前端 `useConversationHistory` 等 hooks 在 execution processes 更新时会反复对全量 process 列表做 sort/flatten/count，并在每条 stream 更新触发大范围派生重算与 rerender。这在 attempt 历史较长或 patch 高频时会显著放大 CPU 与内存占用，造成 UI 卡顿与电量/风扇压力。

## What Changes

- 前端：将 conversation history 派生改为增量式（按 process id 局部更新），避免每次更新全量重建列表。
- 前端：统一 execution processes 的排序/过滤入口，减少多个 hooks/contexts 对同一集合的重复 sort/filter。
- 测试：补齐大数据量场景回归测试（确保不出现明显的 O(n²) 重算路径），并锁定 UI 可见行为（顺序、分组、加载状态）不变。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `frontend-performance-guardrails`: 增加/强化 conversation history 派生的性能约束（增量、稳定引用、避免全量重算）。

## Impact

- Frontend: `frontend/src/hooks/execution-processes/useConversationHistory.ts`、`frontend/src/contexts/ExecutionProcessesContext.tsx`、以及相关派生/渲染路径
- Verification: `pnpm -C frontend run test`, `pnpm -C frontend run lint`, `just qa`, `just openspec-check`

