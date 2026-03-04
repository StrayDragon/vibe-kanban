# archived-kanbans 规范

## 目的

为每个 Project 提供“归档看板批次（ArchivedKanban）”能力：用户可以把当前活跃看板中的一批 tasks 归档到一个只读的静态 Kanban 面板中，以清理活跃看板并保留历史；归档后的 tasks 只能查看与批量还原，禁止任何修改、删除或执行（attempt）。

## 新增需求

### Requirement: ArchivedKanban 为 Project 作用域的归档批次

系统 SHALL 支持创建与查询 `ArchivedKanban`，其作用域为单个 `Project`，并至少包含：
- `id`（UUID）
- `project_id`
- `title`
- `created_at` / `updated_at`

`ArchivedKanban` 在创建后 SHALL 被视为不可变快照容器：系统 MUST NOT 提供更新其元数据（例如重命名）的写路径（删除除外）。

#### Scenario: 列出某个 Project 的归档列表
- **WHEN** 客户端请求某个 Project 的 ArchivedKanban 列表
- **THEN** 系统返回仅属于该 Project 的 ArchivedKanbans，按创建时间排序，并包含每个 archive 的基本元数据与任务计数信息（至少总数）

### Requirement: 归档操作创建 ArchivedKanban 并将任务移动进去

系统 SHALL 提供“一键归档”操作，用于将一批符合条件的 tasks 从活跃集合移动到一个新建的 ArchivedKanban 中。

归档“移动”语义为：对每个被归档的 task，系统将其 `archived_kanban_id` 设置为新建 ArchivedKanban 的 `id`。系统 MUST 仅归档满足以下条件的 tasks：
- `task.project_id` 等于请求的 `project_id`
- `task.archived_kanban_id IS NULL`（仅活跃 tasks 可被归档）
- `task.status` 在请求给定的 `statuses` 集合内

`statuses` MUST 非空；否则系统返回输入错误。

归档操作（创建 ArchivedKanban + 批量移动 tasks）SHALL 在单个数据库事务中完成，以避免出现“创建了 archive 但未移动 tasks”或“移动了部分 tasks”这类中间态。

归档标题 `title` 允许省略或为空白字符串；系统 MUST 将空白标题归一化为“未提供标题”，并生成可读的默认标题。

系统 MUST 拒绝创建空归档：若按过滤条件在目标 Project 内没有任何匹配 tasks，系统 MUST NOT 创建 ArchivedKanban 记录。

系统 MUST 拒绝归档运行中任务：若匹配 tasks 中存在仍有运行中执行进程的 task，系统 MUST 拒绝归档并返回冲突错误；并且 MUST NOT 移动任何 tasks 或创建 ArchivedKanban。

为处理并发/竞态，系统 MUST 在写入阶段执行条件更新，并校验受影响行数与预期一致；否则 MUST 返回冲突错误并不提交事务。

#### Scenario: 归档 done 与 cancelled
- **WHEN** 用户对某个 Project 发起归档请求，`statuses=[done,cancelled]`
- **THEN** 系统创建一个新的 ArchivedKanban，并将该 Project 下所有 `status ∈ {done,cancelled}` 且未归档的 tasks 移动到该 ArchivedKanban

#### Scenario: 归档请求标题为空白时生成默认标题
- **WHEN** 用户发起归档请求且 `title="   "`（仅空白）
- **THEN** 系统创建归档成功，且返回的 `archived_kanban.title` 为服务端生成的默认标题

#### Scenario: 归档拒绝空选择
- **WHEN** 归档请求在目标 Project 内没有任何匹配 tasks（按 `statuses` + 未归档过滤后为空）
- **THEN** 系统返回一个可理解的错误，并且 MUST NOT 创建空的 ArchivedKanban 记录

#### Scenario: 归档拒绝运行中任务
- **WHEN** 归档请求匹配的 tasks 中存在仍有运行中执行进程的 task
- **THEN** 系统拒绝归档并返回冲突错误；并且 MUST NOT 移动任何 tasks 或创建 ArchivedKanban

### Requirement: 任务组（TaskGroup）必须按组原子处理（不确定性安全阀）

当 task 存在 `task_group_id` 时，归档/还原/删除操作 MUST 以“任务组”为最小原子单元：

- **归档：**
  - 若匹配集合命中某个任务组内任一 task，则系统 MUST 将该组内全部 tasks 一并归档（无论其 `status` 是否在请求 `statuses` 内）。
  - 系统 MUST 要求该组在归档前处于一致状态：组内所有 tasks 均为活跃（`archived_kanban_id IS NULL`）。若检测到该组已被拆分（部分已归档），系统 MUST 返回冲突错误并拒绝归档。

- **还原：**
  - 若 restore-by-status 命中某个任务组内任一 task，则系统 MUST 将该组内全部 tasks 一并还原。
  - 系统 MUST 要求该组处于一致归档状态：组内所有 tasks 必须属于同一个 ArchivedKanban。若检测到组被拆分到活跃集合或多个归档，系统 MUST 返回冲突错误并拒绝还原。

- **删除归档：**
  - 若归档包含某个任务组内任一 task，系统 MUST 在删除前检查该组是否完全位于该归档内；若组内存在任一 task 不在该归档（活跃或其他归档），系统 MUST 返回冲突错误并拒绝删除（防止“只删了半组”）。

#### Scenario: 归档命中任务组时整组一起归档
- **WHEN** 归档请求匹配集合中包含某任务组的一部分 tasks
- **THEN** 系统将该任务组内全部 tasks 一并归档到同一个 ArchivedKanban

#### Scenario: 拒绝对已拆分任务组再次归档
- **WHEN** 某任务组存在部分 tasks 已归档、部分仍活跃，且用户再次发起归档
- **THEN** 系统返回冲突错误并拒绝归档

#### Scenario: 拒绝还原被拆分到多个归档的任务组
- **WHEN** 某任务组的 tasks 分别位于多个 ArchivedKanban 中，且用户尝试还原其中任一部分
- **THEN** 系统返回冲突错误并拒绝还原

#### Scenario: 删除归档时拒绝包含拆分任务组
- **WHEN** 某 ArchivedKanban 包含某任务组的一部分 tasks（该组另有 tasks 在活跃集合或其他归档）
- **THEN** 系统返回冲突错误并拒绝删除该 ArchivedKanban

### Requirement: 默认从任务列表与任务流中排除归档任务，并提供显式过滤

系统 SHALL 在默认情况下从任务列表与任务流中排除归档任务：
- `/api/tasks`（HTTP 列表）
- `/api/tasks/stream/ws`（WebSocket JSON Patch 流）

系统 MUST 提供显式过滤参数，以允许客户端：
- 包含归档任务（例如 `include_archived=true`）
- 或仅查询某个 archive 下的 tasks（例如 `archived_kanban_id=<id>`）

当提供 `archived_kanban_id=<id>` 时，系统 MUST 仅返回该归档下的 tasks；此时 `include_archived` 的取值不影响结果。

#### Scenario: 默认列表不包含归档任务
- **WHEN** 客户端请求 tasks 且未显式包含归档
- **THEN** 返回结果只包含 `archived_kanban_id IS NULL` 的 tasks

#### Scenario: 按归档 id 过滤任务
- **WHEN** 客户端请求 tasks 并提供 `archived_kanban_id=<id>`
- **THEN** 返回结果只包含 `archived_kanban_id=<id>` 的 tasks

### Requirement: 任务流必须与过滤条件保持一致（避免幽灵任务）

对于 `/api/tasks/stream/ws`：

- 系统 MUST 以订阅过滤条件生成初始快照（snapshot）。
- 系统 MUST 确保增量更新后，客户端视图中的 tasks 集合与过滤条件一致：
  - 若某 task 不再满足过滤条件，系统 MUST 使其从客户端视图中消失（例如通过 JSON Patch remove，或等价效果）。
  - 系统 MUST NOT 让不匹配过滤条件的 task 在客户端视图中持续存在直到刷新。

#### Scenario: 未包含归档时，任务被归档后从流视图中消失
- **WHEN** 客户端订阅任务流且未包含归档（默认），随后某 task 被归档（其 `archived_kanban_id` 由 `NULL` 变为非 `NULL`）
- **THEN** 客户端在该订阅视图中不再看到该 task

### Requirement: 归档任务不可变且不可执行

系统 MUST 将归档 tasks 视为不可变且不可执行对象：
- 系统 MUST 拒绝对归档 tasks 的任何更新（例如修改 title/description/status 等）
- 系统 MUST 拒绝删除归档 tasks（除“删除归档”这一路径外）
- 系统 MUST 拒绝为归档 tasks 创建新的 attempt / workspace / execution（无论通过 HTTP 还是 MCP）

客户端若要继续修改或执行某 task，必须先通过 restore 将其还原回活跃集合（`archived_kanban_id IS NULL`）。

#### Scenario: 拒绝更新归档任务
- **WHEN** 客户端尝试更新一个 `archived_kanban_id != NULL` 的 task
- **THEN** 系统返回冲突错误并不做任何修改

#### Scenario: 拒绝执行归档任务
- **WHEN** 客户端尝试为一个 `archived_kanban_id != NULL` 的 task 创建 attempt（或 create-and-start）
- **THEN** 系统返回冲突错误并不创建任何执行资源

### Requirement: 批量还原将任务移回活跃集合，且不改变 status

系统 SHALL 支持批量还原 ArchivedKanban 内的 tasks 回到活跃集合，至少支持：
- restore-all：还原该 archive 内全部 tasks
- restore-by-status：仅还原指定 `statuses` 集合内的 tasks（但若命中任务组，则按组原子扩展）

还原操作 MUST 将匹配 tasks 的 `archived_kanban_id` 置为 `NULL`，并且 MUST NOT 改变 task 的 `status`。

系统 MUST 在写入阶段执行条件更新，并校验受影响行数与预期一致；否则 MUST 返回冲突错误并不提交事务。

#### Scenario: 还原全部任务
- **WHEN** 用户对某个 ArchivedKanban 发起 restore-all
- **THEN** 该 archive 内所有 tasks 的 `archived_kanban_id` 变为 `NULL`，并恢复到活跃集合

#### Scenario: 按 status 还原且保持 status 不变
- **WHEN** 用户对某个 ArchivedKanban 发起 restore-by-status（例如 `statuses=[done]`）
- **THEN** 所有被还原的 tasks 的 `status` 值保持不变

#### Scenario: 还原筛选为空时返回 0
- **WHEN** 用户对某个 ArchivedKanban 发起 restore-by-status，但该归档内没有任何匹配 tasks
- **THEN** 系统返回成功响应，且 `restored_task_count=0`

### Requirement: 删除归档会永久删除其包含任务（破坏性）

系统 SHALL 支持删除一个 ArchivedKanban。删除操作是破坏性的：
- 系统 MUST 在删除前检查该 archive 内是否存在运行中执行进程；若存在 MUST 拒绝删除
- 系统 MUST 应用任务组安全阀：若 archive 内存在被拆分到外部的任务组，系统 MUST 拒绝删除
- 若可删除，系统 MUST 使用标准 task 删除清理流程删除该 archive 内所有 tasks（确保资源清理行为与现有删除一致）
- 在 tasks 删除完成后，系统 MUST 删除 ArchivedKanban 记录

#### Scenario: 删除归档会删除其中 tasks
- **WHEN** 用户删除某个 ArchivedKanban，且其中 tasks 均无运行中执行进程，且不存在拆分任务组
- **THEN** 系统永久删除该 archive 及其中所有 tasks，并返回被删除的 task 数量

#### Scenario: 删除归档拒绝运行中任务
- **WHEN** 用户删除某个 ArchivedKanban，但其中任一 task 仍有运行中执行进程
- **THEN** 系统拒绝删除并返回冲突错误，且不删除任何 tasks 或 archive 记录

### Requirement: MCP 提供 archived-kanbans 工具集

系统 SHALL 在 MCP server 中提供 ArchivedKanban 相关工具集，用于：
- 列出 Project 下的 ArchivedKanbans
- 触发归档（创建 archive 并移动 tasks）
- 触发还原（批量还原）
- 删除 archive（破坏性）

这些 tools MUST 提供 `structuredContent`，并在 `tools/list` 中发布准确的 `outputSchema`。破坏性 tools MUST 标注 `destructiveHint=true`。

#### Scenario: MCP tools 返回结构化结果
- **WHEN** 客户端调用任一 archived-kanban MCP tool
- **THEN** tool result 包含 `structuredContent`，其字段语义与 tool 文档一致

