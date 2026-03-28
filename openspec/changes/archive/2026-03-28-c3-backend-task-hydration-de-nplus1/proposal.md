## Why

当前 tasks 列表与 realtime snapshot/resync 会频繁调用 `Task::{find_all,find_by_project_id,find_filtered}_with_attempt_status()`。这些函数对每条 task model 都调用 `with_attempt_status()`，而 `with_attempt_status()` 又会做多次 DB 读取：

- `Task::from_model()`：对 `project_id` / `parent_workspace_id` / `origin_task_id` / `shared_task_id` / `archived_kanban_id` / `milestone_id` 逐个做 `ids::*_uuid_by_id()` 查询（典型 N+1 fan-out）。
- `Task::attempt_status()`：每条 task 3 次查询（running? / latest status / latest executor）。
- `TaskDispatchState::find_by_task_id()`：每条 task 2 次查询（task row id + dispatch_state）。
- `resolve_task_orchestration_diagnostics()`：对 auto-managed candidate 还会额外查询 milestone / project / orchestration_state。

在 snapshot / lag resync / tasks 列表刷新路径下，这会把单次请求放大为 O(N) 次 DB round-trip，导致 CPU、内存分配、以及尾延迟显著上升，并对 DB 造成不必要压力。

## What Changes

- 后端：为 tasks 列表 hydration 引入 **bulk/batched** 路径，将 uuid 解析、attempt status、dispatch state、orchestration diagnostics 的读取改为 **按需最小读取 + 批量查询**，避免逐条 task 的 N+1 查询。
- 行为保持：API 返回字段与语义保持不变（仅内部实现与查询策略调整）。
- 测试：增加覆盖以锁定 attempt status / dispatch state / orchestration diagnostics 的行为与边界输入，避免性能改造引入一致性回归。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

（无，内部实现优化；对外行为不变）

## Impact

- Backend: `crates/db/src/models/task/mod.rs`（核心）、`crates/db/src/models/task_dispatch_state.rs`（必要的可复用构造/批量读取辅助）、以及相关调用方（streams/routes）。
- Verification: `cargo test -p db`, `cargo test --workspace`, `just qa`, `just openspec-check`

