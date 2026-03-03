## Why（为什么）

当前 Vibe Kanban 的任务主要有两个入口：**All Tasks**（全部任务）以及按 Project 展示的 **Kanbans**。用户希望能够快速“一键清理当前看板”，同时保留历史上下文。现在要清空看板只能删除任务，或把历史任务继续混在活跃任务里，时间久了会让看板越来越嘈杂、难以聚焦当前工作。

因此需要引入一个一等公民的、只读的“归档看板快照”概念：用户可以将一批任务归档到一个 **ArchivedKanban**（静态 Kanban 布局面板）中，之后需要时还能批量还原。

## What Changes（会改什么）

- 引入 **ArchivedKanban** 概念：Project 作用域的归档看板，包含从活跃看板“移动”过去的一批任务。
- 在 Project Kanban 视图新增 **Archive** 一键归档动作：
  - 用户选择要归档哪些 `status`（默认：`done`、`cancelled`；可配置）。
  - 系统创建新的 ArchivedKanban，并将匹配的任务移动到其中，从而从活跃看板中移除这些任务。
- 新增 **Archives** UI：
  - 列出某 Project 下的 ArchivedKanbans。
  - ArchivedKanban 详情页以 Kanban 列布局展示任务，但**完全只读**（不可拖拽、不可编辑）。
- 新增 **Restore** 还原动作：
  - 支持按选定 `status` 或 restore-all 的方式批量还原任务回活跃看板。
- 支持 **硬删除** ArchivedKanban：
  - 删除 ArchivedKanban 会永久删除其中任务（前端必须提供强确认与明确风险提示）。
- 强制不可变性（immutability）：
  - ArchivedKanban 内的任务不可更新、不可删除、不可执行（不可创建 attempt）；若要继续操作必须先还原。
- 调整任务列表/流式默认行为：
  - **All Tasks** 与 Project **Kanbans** 默认不包含归档任务；需要时通过显式 toggle/query 参数包含归档数据。
- 增加 MCP 聚合：
  - 新增 MCP tools 用于列出/归档/还原/删除 ArchivedKanbans，语义与 HTTP 路由对齐。

## Capabilities（能力）

### New Capabilities（新增能力）
- `archived-kanbans`：Project 作用域的归档看板快照，提供 archive/restore/delete 流程、严格只读语义，以及 MCP/HTTP/UI 支持。

### Modified Capabilities（修改既有能力）
- （无）

## Impact（影响面）

- 数据库 / 模型：
  - 新增 `archived_kanbans` 表（Project 作用域）。
  - 为 `tasks` 增加可空外键 `archived_kanban_id`，将任务关联到某个 ArchivedKanban。
- 后端 API：
  - 新增路由：`/api/projects/:project_id/archived-kanbans` 与 `/api/archived-kanbans/:id/*`。
  - 扩展 `/api/tasks` 与 `/api/tasks/stream/ws` 的 query 语义，用于过滤/包含归档任务。
  - 增加服务端 guard：阻止归档任务的 update/delete/attempt 创建。
- MCP：
  - 为 `mcp_task_server` 增加 archived-kanban 工具（只读列举 + 可控的破坏性操作）。
- 前端：
  - 新增 Project 作用域路由：`/projects/:projectId/archives` 与 `/projects/:projectId/archives/:archiveId`。
  - 增加 Archive/Restore/Delete 对话框（强确认、清晰警告）。
- 类型：
  - 更新 Rust 类型并通过 `pnpm run generate-types` 重新生成 `shared/types.ts`（禁止直接编辑生成文件）。

## Goals（目标）

- 用户可以一键把当前 Project 看板的一批任务归档到新的 ArchivedKanban 中。
- ArchivedKanban 页面静态只读，适合安全地浏览历史。
- 归档任务在服务端强制不可变、不可执行，直到被还原。
- 还原支持批量且行为可预测。
- All Tasks 默认仅展示活跃任务，避免历史噪声。

## Non-goals（非目标）

- 不引入“Milestone 里程碑规划”能力（日期、目标、进度等）。
- 不支持单 Project 下多个并行活跃 Kanban。
- 不保留列内任务顺序（只保证 `status` 阶段准确即可）。
- 不做跨 Project 的归档分组（仅 Project 内归档）。
- v1 不提供逐个 task 勾选的归档/还原 UI（按 `status` 批量足够）。

## Risks（风险）

- 删除 ArchivedKanban 会导致不可恢复的数据丢失（包含其内部任务）→ 必须提供强确认与清晰警告文案。
- 客户端对 “All Tasks” 默认行为的预期可能变化 → 默认过滤策略需要显式参数与文档说明。
- Guardrail 覆盖不完整会导致历史被改写 → 需要覆盖所有写路径并加测试。

## Verification（验证）

- 后端测试：
  - 归档只移动选定状态的任务，并且归档任务默认不会出现在任务列表/流中。
  - 对归档任务的更新、删除与 attempt 创建会被服务端拒绝。
  - 还原会把任务放回活跃集合且不改变 `status`。
  - 删除归档会永久删除归档内任务并执行标准清理流程。
- 前端检查：
  - Archive/restore/delete 流程可用，archives 页面只读。
  - All Tasks 的 “include archived” 开关能正确包含/排除归档任务。
