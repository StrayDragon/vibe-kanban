## Context

目前 archived kanban tasks 页面通过 `useArchivedKanbanTasks` 订阅 `/api/tasks/stream/ws?archived_kanban_id=...&include_archived=true`，但在每次 patch 后都会执行全量派生：
- `Object.values(tasksById)` 全量扫描
- 全量 `sort`（按 `created_at`）
- 对每个 status 的 list 再全量 `sort`
- 额外的 `{ ...tasksById }` 合并/复制

这在 task 数量较多或更新频率较高时会造成明显的 CPU/alloc 峰值；同时也会破坏组件 memo 的收益（数组/对象每次都是新引用）。

项目已经在 `useAllTasks` / `useProjectTasks` 中引入了 `taskDerivation` 增量派生缓存，并通过 WS `invalidate.taskIds` 只对变更 id 进行更新。本变更将把同样的模式应用到 archived kanban tasks，形成一致的性能基线。

## Goals / Non-Goals

**Goals:**
- archived kanban tasks 的 `tasks` / `tasksByStatus` 派生改为增量更新：仅对 `invalidate.taskIds` 涉及的 id 做插入/删除/替换/跨 status 移动。
- 派生结果保持稳定排序（`created_at` desc，tie-break by `id`），并尽可能维持未受影响 list 的引用稳定（帮助组件 memo/避免额外渲染）。
- 增加单测覆盖：验证排序/分桶语义、以及“单任务同 status 更新时，其他 status list 引用不变”的契约。

**Non-Goals:**
- 不在本 change 中重写 `TasksOverview` / `ProjectTasks` 页面层派生与渲染（另起 change 再做）。
- 不改变 WS 协议或后端行为（已依赖现有 `invalidate` hints）。
- 不引入新的前端状态管理方案或新增依赖。

## Decisions

1) **复用 `taskDerivation` 的缓存与增量更新算法**
- 方案：对 archived tasks 的 map 使用 `buildTaskDerivationCache(Object.values(map))` 初始化；后续基于 `invalidate.taskIds` 调用 `applyTaskDerivationChanges(cache, changedIds, getTask)`。
- 原因：该算法已在 `useAllTasks` / `useProjectTasks` 中验证；能避免全量排序并提供引用稳定性（只 clone 受影响的数组）。
- 备选：在 `useArchivedKanbanTasks` 内部手写增量逻辑（更贴合场景但重复实现、易引入差异）。

2) **当无法安全增量更新时回退 full rebuild**
- 方案：当 `changedIds` 为空、cache 不存在、或 `applyTaskDerivationChanges` 返回 false（不满足假设）时执行 full rebuild。
- 原因：保持正确性优先；性能优化必须有兜底路径。

## Risks / Trade-offs

- [增量更新遗漏边界条件] → 通过单测覆盖插入/删除/跨 status 移动，并保留 full rebuild 兜底。
- [引用稳定性契约过强导致后续难以调整] → 将契约限定在可观测行为（未受影响 list 引用稳定），并在 spec 中明确仅适用于 id-map patch 的局部更新场景。

