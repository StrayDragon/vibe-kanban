## Context

`TaskWithAttemptStatus` 是 tasks 列表与 realtime tasks stream 的基础数据结构。当前 list/hydration 路径将单次列表查询扩大为“每条 task 多次查询”的 DB fan-out：

- ids uuid 解析：每条 task 多次 `ids::*_uuid_by_id()`
- attempt status：每条 task 3 次查询
- dispatch state：每条 task 2 次查询
- orchestration diagnostics：对候选 task 再额外查询 milestone/project/state

这在 snapshot 或 lag resync 时会被放大，出现 CPU/alloc 抖动与 DB 压力。

本变更目标：在不改变对外行为的前提下，把任务列表 hydration 调整为 **O(1) 次查询（按表常数级）**。

## Goals / Non-Goals

**Goals**
- 列表查询不再 per-row 调 `with_attempt_status()`。
- uuid 解析改为按需批量解析（每张表最多一次查询）。
- attempt status / executor / dispatch_state / orchestration_state 改为批量查询与映射。
- 保持 JSON 输出字段/默认值与现有行为一致（例如 executor 缺失时返回空字符串）。

**Non-Goals**
- 不修改 DB schema。
- 不引入跨请求缓存或改变业务语义。
- 不改变 routes / realtime wire shape。

## Decisions

### 1) 新增 `hydrate_with_attempt_status_bulk(models)` 作为唯一汇聚点

- 输入：`Vec<task::Model>`（已经完成 list filter/order）。
- 输出：`Vec<TaskWithAttemptStatus>`，顺序与输入 models 保持一致。
- 该函数内部完成：
  - task foreign ids → uuids 的批量映射
  - attempt status 的批量计算
  - dispatch_state 的批量读取
  - orchestration diagnostics 的批量读取（仅对候选 task）

### 2) uuid 解析：对每张外键表做一次 `WHERE id IN (...)` 映射

将 `Task::from_model()` 的逐条查询替换为：
- `project(id -> uuid)`
- `workspace(id -> uuid)`（parent_workspace）
- `task(id -> uuid)`（origin_task）
- `shared_task(id -> uuid)`
- `archived_kanban(id -> uuid)`
- `milestone(id -> uuid)`

并在构造 `Task` 时使用 HashMap 查表；缺失映射按现有行为返回 `RecordNotFound`（保护 referential integrity）。

### 3) attempt status：用 3 条批量查询替代 per-task 3 条查询

- `has_in_progress_attempt`：查询所有 `status=Running` 且 `run_reason in {SetupScript,CleanupScript,CodingAgent}` 的 task_row_id 集合。
- `last_attempt_failed`：按 task 聚合找到最新 execution_process（按 created_at + id 断言稳定），读取其 status 后映射为 bool。
- `executor`：按 task 聚合找到最新 session（按 created_at + id），读取 executor（NULL → 空字符串）。

### 4) dispatch_state：按 task_row_id 一次性读取并映射为 `TaskDispatchState`

`task_dispatch_state` 以 task row id 为外键；按 `WHERE task_id IN (...)` 拉取后，以 row_id → task_uuid 映射构造。

### 5) orchestration diagnostics：只对候选 task 批量读取

候选条件（与现有逻辑一致）：
- `milestone_id` 非空
- `task_kind != Milestone`
- `milestone_node_id` 非空且非空白

对候选任务：
- 批量读取 milestone 的 `automation_mode`（按 milestone row id）
- 批量读取 project 的 `default_continuation_turns`（按 project row id）
- 批量读取 `task_orchestration_state`（按 task_row_id）

仅当 milestone 为 Auto 时计算 `TaskOrchestrationDiagnostics`，否则返回 None。

## Risks / Trade-offs

- [Tie-breaking 差异] 聚合查询需要明确“最新”判定，使用 `(created_at, id)` 做稳定 tie-break，避免与旧逻辑（order by created_at desc limit 1）出现不确定差异。
- [实现复杂度上升] 控制复杂度在 `task/mod.rs` 局部 helper 内，避免跨 crate 大重构；并用单测锁定输出行为。

