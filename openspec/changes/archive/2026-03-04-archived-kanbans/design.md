## 背景

当前 Vibe Kanban 的核心数据模型包括：

- `Project`：一个逻辑单元，聚合一个或多个 Git 仓库（`project_repos`），并拥有其下的 `Task`。
- `Task`：属于某个 `Project`，并具有 `status`（`todo|inprogress|inreview|done|cancelled`）。
- Project 的 Kanban UI 按 `status` 分列展示任务；拖拽目前只会更新 `Task.status`。

系统目前没有一等公民的 Kanban 实体，也没有列内顺序的持久化。用户希望把一批任务归档到一个静态、只读的 Kanban 面板（ArchivedKanban）中，以保持活跃 Project 看板干净，同时保留历史，并支持批量还原。

已确认的产品约束：

- `Project` / `Repo` 保持现有含义，不把 `Project` 重新定义为归档概念。
- ArchivedKanban 为 Project 作用域。
- 不要求保留“布局顺序”，只要求阶段（`status`）准确。
- 归档任务完全不可变、不可执行：不允许 update/delete/start attempt；只允许查看与还原。
- 还原必须支持批量（restore-all 或按 `status` 还原）。
- 允许删除 ArchivedKanban，且删除会永久删除其包含的任务（UI 需强确认）。
- All Tasks 默认排除归档任务。

## 目标与非目标

**目标：**
- 引入 `ArchivedKanban` 领域模型，用于表示某个 Project 下的一次归档批次（归档看板）。
- 通过 HTTP + UI 提供 archive/restore/delete 流程，并提供 MCP tools 供自动化使用。
- 在服务端层面强制执行“归档 = 不可变 + 不可执行”，而不是仅依赖前端只读。
- 对未归档任务保持现有 Project/Repo 与活跃看板行为不变。

**非目标：**
- 不引入规划型里程碑能力（日期、进度跟踪等）。
- 不支持单 Project 下多个并行活跃 Kanban。
- 不持久化列内顺序，也不做“冻结拖拽顺序”的展示。
- 不做跨 Project 的归档聚合。
- v1 不提供逐个 task 勾选的归档/还原 UI（按 `status` 批量足够）。

## 关键决策

### 1) 用新表 + `tasks` 可空外键建模归档

**决策：** 新增 `archived_kanbans` 表，并在 `tasks` 上新增 `archived_kanban_id`（nullable FK）。

**原因：**
- 保持 Project/Repo 的既有语义，同时让归档批次拥有独立身份（title、时间戳等）。
- 支持同一个 Project 下存在多个归档批次。
- 查询高效清晰：活跃任务满足 `archived_kanban_id IS NULL`；归档详情满足 `archived_kanban_id = X`。

**备选方案：**
- 用 Tag 做归档：缺少强容器语义，且不自然支持“删除归档=删除其 tasks”。
- 用新的 `TaskStatus` 表示归档：会改变 status 列的含义，破坏现有 Kanban UI 预期。
- 用 join 表（`archived_kanban_tasks`）：更灵活但 v1 不需要；FK 更简单直接。

### 2) 服务端强制“归档任务不可变 + 不可执行”

**决策：** 任何会修改 task 或启动执行的 server endpoint / MCP tool，都必须以一致错误拒绝归档任务（例如 HTTP 409 Conflict / MCP structured error）。

**原因：**
- 即使未来 UI 新增编辑入口或有客户端绕过 UI，也能保护历史不被改写。
- 让 archived 语义明确、可预测。

### 3) 任务列表/任务流默认排除归档任务

**决策：** `/api/tasks` 与 `/api/tasks/stream/ws` 默认只返回活跃任务；需要时通过显式过滤参数包含归档任务或查询某个归档批次（`include_archived`、`archived_kanban_id`）。

**原因：**
- All Tasks 与活跃 Kanbans 更聚焦当前工作。
- 归档体验是增量能力，不会让默认体验退化。

### 4) `task_group_id` 原子化 + 安全阀（处理不确定性）

**决策：**
- 归档/还原/删除任一操作只要命中某个任务组（`task_group_id`），就按“整组”为最小原子单元执行。
- 检测到同一任务组被拆分到活跃集合与归档集合（或多个归档）中，直接返回冲突错误，拒绝继续（安全阀）。

**原因：**
- 任务组的意义是“一组节点共同构成一个工作单元”。对组进行部分归档/部分还原/部分删除会制造隐性数据丢失。
- 拒绝“组拆分”能把未知状态显性化，并把修复动作交回给用户（先 restore 到一致状态再继续）。

### 5) 归档标题可选，服务端生成默认标题

**决策：** `title` 允许省略或传入空白字符串；服务端将其归一化为空并生成默认标题（例如 `归档 2026-03-04 12:34`）。

**原因：**
- 降低用户操作成本，同时保证每个归档可识别。

### 6) 删除归档为同步硬删除，复用标准任务清理逻辑

**决策：** 删除 ArchivedKanban 时，永久删除其中 tasks，并复用标准任务删除清理流程（workspaces/containers 等），最后删除 archive 记录。

**原因：**
- 避免留下 workspace/container 等外部资源泄漏。
- 删除语义与既有任务删除路径一致。

### 7) 任务流过滤需要收敛增量 patch，避免“幽灵任务”

**决策：**
- 初始快照严格按过滤条件生成。
- 对于增量事件：若 patch 携带的 task 不匹配订阅过滤条件，服务端将其收敛为“从客户端视图移除该 task”的效果。
- 当流 lagged 时，以同一过滤条件重建快照进行 resync。

**原因：**
- 过滤视图必须自洽：订阅不包含归档时，任务被归档后应立即从视图消失，而不是残留直到刷新。

### 8) MCP 工具保持单一职责与严格 schema

**决策：** 增加离散 MCP tools：
- `list_archived_kanbans`
- `archive_project_kanban`
- `restore_archived_kanban`

**原因：**
- 与现有 MCP 设计一致（小工具、严格 schema、结构化输出）。
- 写操作通过 `destructiveHint=true` 明示风险。

## 风险与权衡

- **删除归档导致不可恢复的数据丢失** → UI 高摩擦确认（输入确认文本）、醒目文案说明永久删除；MCP 不暴露 delete tool。
- **护栏覆盖不完整（遗漏写路径）** → 将“是否归档”的判断尽量集中，并为每类写操作（update/delete/create-and-start/start_attempt）增加针对性测试。
- **删除可能较慢/外部清理可能部分失败** → 返回清晰错误 + best-effort 清理；必要时再演进为后台队列。
- **默认过滤改变旧客户端预期** → 通过显式 query 参数提供包含归档/按归档筛选能力，并同步更新内部调用点与文档。

## 不确定性与未知情况处理（补充）

为了在并发、竞态、数据异常等不可预期情况下保持语义可预测，本方案补充以下策略：

- **条件更新 + 行数校验**：归档/还原写入时只更新仍满足前置条件的 tasks，并校验受影响行数；不一致则返回冲突错误，提示刷新后重试。
- **任务组拆分检测**：任何导致任务组跨活跃/归档或跨多个归档的状态，视为冲突并拒绝继续。
- **运行中进程拒绝**：归档/删除归档均拒绝包含运行中执行进程的任务。
- **流 lagged 的快照重建**：当 WS lagged 时，按过滤条件重建 snapshot，避免客户端进入未知中间态。

## 迁移计划

1. 新增 SeaORM migration：
   - 创建 `archived_kanbans` 表并设置 Project FK。
   - 为 `tasks` 增加 `archived_kanban_id` nullable FK + index。
2. 增加 db models 与 server routes；重新生成 TypeScript types。
3. 增加前端页面与对话框，接入新 API。
4. 增加 MCP tools（list/archive/restore），并确保输出包含 `outputSchema` 与 `structuredContent`。

**回滚：** 删除新增表/列并移除路由/工具/UI。若系统中已存在归档数据，回滚会丢失归档分组语义，属于非平凡回滚。

## 开放问题

- 是否需要在 restore 请求中支持 `task_ids[]`（除 restore-by-status 之外）？v1 不要求，但可以在请求结构中预留扩展空间。
