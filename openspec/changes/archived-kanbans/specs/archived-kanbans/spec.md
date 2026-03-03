# archived-kanbans Specification

## Purpose
为每个 Project 提供“归档看板批次（ArchivedKanban）”能力：用户可以把当前活跃看板中的一批 tasks 归档到一个只读的静态 Kanban 面板中，以清理活跃看板并保留历史；归档后的 tasks 只能查看与批量还原，禁止任何修改、删除或执行（attempt）。

## ADDED Requirements

### Requirement: ArchivedKanban is a Project-scoped archive batch
系统 SHALL 支持创建与查询 `ArchivedKanban`，其作用域为单个 `Project`，并至少包含：
- `id`（UUID）
- `project_id`
- `title`
- `created_at` / `updated_at`

`ArchivedKanban` 在创建后 SHALL 被视为不可变快照容器：系统 MUST NOT 提供更新其元数据（例如重命名）的写路径（删除除外）。

#### Scenario: List archives for a project
- **WHEN** 客户端请求某个 Project 的 ArchivedKanban 列表
- **THEN** 系统返回仅属于该 Project 的 ArchivedKanbans，按创建时间排序，并包含每个 archive 的基本元数据与任务计数信息（至少总数）

### Requirement: Archive creates an ArchivedKanban and moves tasks into it
系统 SHALL 提供“一键归档”操作，用于将一批符合条件的 tasks 从活跃看板移动到一个新建的 ArchivedKanban 中。

归档“移动”语义为：对每个被归档的 task，系统将其 `archived_kanban_id` 设置为新建 ArchivedKanban 的 `id`。系统 MUST 仅归档满足以下条件的 tasks：
- `task.project_id` 等于请求的 `project_id`
- `task.archived_kanban_id IS NULL`（仅活跃 tasks 可被归档）
- `task.status` 在请求给定的 `statuses` 集合内

归档操作（创建 ArchivedKanban + 批量移动 tasks）SHALL 在单个数据库事务中完成，以避免出现“创建了 archive 但未移动 tasks”或“移动了部分 tasks”这类中间态。

#### Scenario: Archive done and cancelled tasks
- **WHEN** 用户对某个 Project 发起归档请求，`statuses=[done,cancelled]`
- **THEN** 系统创建一个新的 ArchivedKanban，并将该 Project 下所有 `status ∈ {done,cancelled}` 且未归档的 tasks 移动到该 ArchivedKanban

#### Scenario: Archive rejects empty selection
- **WHEN** 归档请求在目标 Project 内没有任何匹配 tasks（按 `statuses` + 未归档过滤后为空）
- **THEN** 系统返回一个可理解的错误，并且 MUST NOT 创建空的 ArchivedKanban 记录

#### Scenario: Archive rejects running tasks
- **WHEN** 归档请求匹配的 tasks 中存在仍有运行中执行进程的 task
- **THEN** 系统拒绝归档并返回冲突错误；并且 MUST NOT 移动任何 tasks 或创建 ArchivedKanban

### Requirement: Archived tasks are excluded by default from task listings and streams
系统 SHALL 在默认情况下从任务列表与任务流中排除归档任务：
- `/api/tasks`（HTTP 列表）
- `/api/tasks/stream/ws`（WebSocket JSON Patch 流）

系统 MUST 提供显式过滤参数，以允许客户端：
- 包含归档任务（例如 `include_archived=true`）
- 或仅查询某个 archive 下的 tasks（例如 `archived_kanban_id=<id>`）

#### Scenario: Default list excludes archived
- **WHEN** 客户端请求某个 Project 的 tasks 且未显式包含归档
- **THEN** 返回结果只包含 `archived_kanban_id IS NULL` 的 tasks

#### Scenario: Filter by archive id
- **WHEN** 客户端请求 tasks 并提供 `archived_kanban_id=<id>`
- **THEN** 返回结果只包含 `archived_kanban_id=<id>` 的 tasks

### Requirement: Archived tasks are immutable and non-executable
系统 MUST 将归档 tasks 视为不可变且不可执行对象：
- 系统 MUST 拒绝对归档 tasks 的任何更新（例如修改 title/description/status 等）
- 系统 MUST 拒绝删除归档 tasks
- 系统 MUST 拒绝为归档 tasks 创建新的 attempt / workspace / execution（无论通过 HTTP 还是 MCP）

客户端若要继续修改或执行某 task，必须先通过 restore 将其还原回活跃集合（`archived_kanban_id IS NULL`）。

#### Scenario: Reject update on archived task
- **WHEN** 客户端尝试更新一个 `archived_kanban_id != NULL` 的 task
- **THEN** 系统返回冲突错误并不做任何修改

#### Scenario: Reject execution on archived task
- **WHEN** 客户端尝试为一个 `archived_kanban_id != NULL` 的 task 创建 attempt（或 create-and-start）
- **THEN** 系统返回冲突错误并不创建任何执行资源

### Requirement: Restore moves tasks back to the active set in batch
系统 SHALL 支持批量还原 ArchivedKanban 内的 tasks 回到活跃集合，至少支持：
- restore-all：还原该 archive 内全部 tasks
- restore-by-status：仅还原指定 `statuses` 集合内的 tasks

还原操作 MUST 将匹配的 tasks 的 `archived_kanban_id` 置为 `NULL`，并且 MUST NOT 改变 task 的 `status`。

#### Scenario: Restore all tasks
- **WHEN** 用户对某个 ArchivedKanban 发起 restore-all
- **THEN** 该 archive 内所有 tasks 的 `archived_kanban_id` 变为 `NULL`，并恢复到活跃集合

#### Scenario: Restore by status preserves status
- **WHEN** 用户对某个 ArchivedKanban 发起 restore-by-status（例如 `statuses=[done]`）
- **THEN** 仅 `status=done` 的 tasks 被还原，并且其 `status` 值保持不变

### Requirement: Delete archive permanently deletes contained tasks
系统 SHALL 支持删除一个 ArchivedKanban。删除操作是破坏性的：
- 系统 MUST 在删除前检查该 archive 内是否存在运行中执行进程；若存在 MUST 拒绝删除
- 若可删除，系统 MUST 使用标准 task 删除清理流程删除该 archive 内所有 tasks（确保资源清理行为与现有删除一致）
- 在 tasks 删除完成后，系统 MUST 删除 ArchivedKanban 记录

#### Scenario: Delete archive removes tasks
- **WHEN** 用户删除某个 ArchivedKanban，且其中 tasks 均无运行中执行进程
- **THEN** 系统永久删除该 archive 及其中所有 tasks，并返回被删除的 task 数量

#### Scenario: Delete archive rejects running tasks
- **WHEN** 用户删除某个 ArchivedKanban，但其中任一 task 仍有运行中执行进程
- **THEN** 系统拒绝删除并返回冲突错误，且不删除任何 tasks 或 archive 记录

### Requirement: MCP tool support for archived-kanbans
系统 SHALL 在 MCP server 中提供 ArchivedKanban 相关工具集，用于：
- 列出 Project 下的 ArchivedKanbans
- 触发归档（创建 archive 并移动 tasks）
- 触发还原（批量还原）
- 删除 archive（破坏性）

这些 tools MUST 提供 `structuredContent`，并在 `tools/list` 中发布准确的 `outputSchema`。破坏性 tools MUST 标注 `destructiveHint=true`。

#### Scenario: MCP tools return structured content
- **WHEN** 客户端调用任一 archived-kanban MCP tool
- **THEN** tool result 包含 `structuredContent`，其字段语义与 tool 文档一致

