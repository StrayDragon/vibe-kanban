## Why

前端在 `useAllTasks` / `useProjectTasks` 中，为了处理 optimistic 状态可能“卡住”导致的 resync，会在存在 optimistic state 时启动一个 250ms 的循环 tick（`setTimeout` 自触发）。

这会导致：

- 在 optimistic 状态持续存在的窗口内（哪怕不需要立刻 resync）也会固定频率唤醒；
- 在低功耗设备/长时间运行时造成不必要的 CPU wakeup 与渲染链路抖动；
- resync 的触发条件本质是基于 meta 时间窗（`setAt + resyncAfterMs` / `lastResyncAt + minResyncGapMs`），无需高频轮询。

## What Changes

- 将 optimistic-stale 检查从固定 250ms tick 改为“按需 one-shot timer”：
  - 计算下一次可能满足 resync 条件的最早时间点；
  - 仅在该时间点（或更晚）触发一次检查；
  - 如果仍有 optimistic state 且需要后续检查，再调度下一次 one-shot。
- 语义保持一致：resync 的阈值、最大尝试次数、最小重试间隔、以及实际候选筛选逻辑不变；仅减少无意义唤醒。
- 增加测试覆盖：对“下一次调度时间”的计算逻辑做单测，确保边界条件（attempts 已满 / lastResyncAt 门控等）正确。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `frontend-performance-guardrails`: 增加性能护栏——optimistic-stale 检查必须按需调度，不应依赖固定高频轮询。

## Impact

- Frontend: `frontend/src/hooks/tasks/useAllTasks.ts`
- Frontend: `frontend/src/hooks/projects/useProjectTasks.ts`
- Frontend tests: 新增轻量单测覆盖调度逻辑（Vitest）

## Goals

- 显著降低 optimistic state 存在期间的 CPU wakeup 与无效 render 触发。
- 保持现有 resync 行为与可靠性不变。

## Non-goals

- 不改变 websocket stream 协议、不改变后端行为。
- 不重写 optimistic store 结构或 task derivation 逻辑。

## Risks

- 计时器调度边界错误可能导致 resync 触发变慢/变快 → 通过提取纯函数并用单测覆盖关键边界。

## Verification

- 前端单测：`pnpm -C frontend test`
- 构建：`pnpm -C frontend build`
- 全量：`just qa`、`just openspec-check`
