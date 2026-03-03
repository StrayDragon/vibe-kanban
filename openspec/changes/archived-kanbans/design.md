## Context（背景）

当前 Vibe Kanban 的核心数据模型包括：

- `Project`：一个逻辑单元，聚合一个或多个 Git 仓库（`project_repos`），并拥有其下的 `Task`。
- `Task`：属于某个 `Project`，并具有 `status`（`todo|inprogress|inreview|done|cancelled`）。
- Project 的 “Kanbans” UI 按 `status` 分列展示任务；拖拽目前只会更新 `Task.status`。

系统目前没有一等公民的 “Kanban 实体”，也没有列内顺序的持久化。用户希望把一批任务归档到一个静态、只读的 Kanban 面板（**ArchivedKanban**）中，以保持活跃 Project 看板干净，同时保留历史，并支持批量还原。

已确认的产品约束：

- `Project` / `Repo` 保持现有含义，不把 `Project` 重新定义为归档概念。
- ArchivedKanban 为 Project 作用域。
- 不要求保留“布局顺序”，只要求阶段（status）准确。
- 归档任务完全不可变、不可执行：
  - 不允许 update/delete/start attempt；只允许查看与还原。
- 还原必须支持批量（restore-all 或按 status 还原）。
- 允许删除 ArchivedKanban，且删除会永久删除其包含的任务（UI 需强确认）。
- All Tasks 默认排除归档任务。

## Goals / Non-Goals（目标 / 非目标）

**Goals（目标）：**
- 引入 `ArchivedKanban` 领域模型，用于表示某个 Project 下的一次归档批次（归档看板）。
- 通过 HTTP + UI 提供 archive/restore/delete 流程，并提供 MCP tools 供自动化使用。
- 在服务端层面强制执行“归档 = 不可变 + 不可执行”，而不是仅依赖前端只读。
- 对未归档任务保持现有 Project/Repo 与活跃看板行为不变。

**Non-Goals（非目标）：**
- 不引入规划型里程碑能力（日期、进度跟踪等）。
- 不支持单 Project 下多个并行活跃 Kanban。
- 不持久化列内顺序，也不做“冻结拖拽顺序”的展示。
- 不做跨 Project 的归档聚合。
- v1 不提供逐个 task 勾选的归档/还原 UI（按 status 批量足够）。

## Decisions（关键决策）

### 1) 用一张新表 + `tasks` 上的可空外键来建模归档

**Decision（决策）：** 新增 `archived_kanbans` 表，并在 `tasks` 上新增 `archived_kanban_id`（nullable FK）。

**Why this option（为什么选它）：**
- 保持 Project/Repo 的既有语义，同时让归档批次拥有独立身份（title、时间戳等）。
- 支持同一个 Project 下存在多个归档批次。
- 查询高效清晰：活跃任务满足 `archived_kanban_id IS NULL`；某个归档详情满足 `archived_kanban_id = X`。

**Alternatives considered（替代方案）：**
- 用 `Tag` 做归档：缺少强容器语义，且不自然支持“删除归档=删除其 tasks”。
- 用新的 `TaskStatus` 表示归档：会改变 status 列的含义，破坏现有 Kanban UI 预期。
- 用 join 表（`archived_kanban_tasks`）：更灵活但 v1 不需要；FK 更简单直接。

### 2) 服务端强制归档任务不可变 + 不可执行

**Decision（决策）：** 任何会修改 task 或启动执行的 server endpoint / MCP tool，都 MUST 以一致错误拒绝归档任务（例如 HTTP 409 Conflict / MCP structured error）。

**Why this option（为什么选它）：**
- 即使未来 UI 新增编辑入口或有客户端绕过 UI，也能保护历史不被改写。
- 让 “archived” 语义明确、可预测。

**Alternatives considered（替代方案）：**
- 仅靠前端只读：不安全，API 仍可修改历史。
- 部分可变（允许改文本）：与“快照”语义冲突，并让审计变复杂。

### 3) 任务列表/流默认排除归档任务

**Decision（决策）：** `/api/tasks` 与 `/api/tasks/stream/ws` 默认只返回活跃任务；需要时通过显式过滤参数包含归档任务或查询某个归档批次。

**Why this option（为什么选它）：**
- All Tasks 与活跃 Kanbans 更聚焦当前工作。
- 归档体验是增量能力，不会让默认体验退化。

**Alternatives considered（替代方案）：**
- 永远包含归档：噪声变大，削弱“清理看板”的动机。

### 4) v1 不引入顺序持久化

**Decision（决策）：** v1 不增加列内顺序字段（`position/rank`）。ArchivedKanban 按 status 分列展示，使用现有排序（例如 `created_at`）。

**Why this option（为什么选它）：**
- 与产品要求一致（不关心顺序，只关心阶段准确）。
- 降低 schema 与 UI 复杂度。

### 5) 删除归档是破坏性操作，复用标准任务删除清理逻辑

**Decision（决策）：** 删除 ArchivedKanban 时，永久删除其中 tasks，并复用现有的任务删除清理逻辑（workspaces/containers 等），最后删除 archive 记录。

**Why this option（为什么选它）：**
- 避免留下 workspace/container 等外部资源泄漏。
- 删除语义与既有任务删除路径一致。

**Alternatives considered（替代方案）：**
- 直接 DB delete（快）但可能泄漏外部资源。
- 引入后台队列异步删除（更稳）但超出 v1 范围。

### 6) MCP 工具面向单一职责、可重试的小工具集

**Decision（决策）：** 增加离散 MCP tools：
- `list_archived_kanbans`
- `archive_project_kanban`
- `restore_archived_kanban`
- `delete_archived_kanban`

**Why this option（为什么选它）：**
- 符合现有 MCP 设计指导（小工具、严格 schema）。
- 避免难以幂等/难以重试的“mega-tool”。

## Risks / Trade-offs（风险 / 权衡）

- **归档删除导致数据丢失** → 强确认（输入确认文本）、destructive tool 注解、醒目文案说明永久删除。
- **Guardrail 覆盖不完整（遗漏某条写路径）** → 尽量集中“是否归档”的判断，并为每类写操作（update/delete/create-and-start/start_attempt）增加针对性测试。
- **删除可能较慢/外部清理可能部分失败** → 以清晰错误反馈 + best-effort 清理为主，保持操作显式且由用户触发。
- **现有客户端对列表接口的预期变化** → 增加显式 query 参数（`include_archived`、`archived_kanban_id`），并更新内部调用与文档说明。

## Migration Plan（迁移计划）

1. 新增 SeaORM migration：
   - 创建 `archived_kanbans` 表并设置 Project FK。
   - 为 `tasks` 增加 `archived_kanban_id` nullable FK + index。
2. 增加 db models 与 server routes；重新生成 TypeScript types。
3. 增加前端页面与对话框，接入新 API。
4. 增加 MCP tools，并确保输出包含 `outputSchema` 与 `structuredContent`。

**Rollback（回滚）：** 删除新增表/列并移除路由/工具/UI。若系统中已存在归档数据，回滚会丢失归档分组语义，属于非平凡回滚。

## Open Questions（开放问题）

- 是否需要在 restore 请求中支持 `task_ids[]`（除 restore-by-status 之外）？v1 不要求，但可以在请求结构中预留扩展空间。
