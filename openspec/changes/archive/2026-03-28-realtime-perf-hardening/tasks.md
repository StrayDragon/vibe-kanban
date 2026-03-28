## 1. Backend：tasks patch 生成与 invalidation hints

- [x] 1.1 将 `crates/events/src/lib.rs` 的 `emit_task_patch` 改为 `Task::find_by_id_with_attempt_status` 的按需读取（不再扫描整项目任务列表），并补回归测试（`cargo test -p events`）
- [x] 1.2 在 `crates/logs-axum/src/lib.rs` 实现基于 `json_patch::Patch` 的 `invalidate` hints 生成（避免 `serde_json::to_value` 往返），并补齐单测覆盖 tasks/workspaces/execution_processes + `~0/~1` 转义（`cargo test -p logs-axum`）
- [x] 1.3 增强 WS message 契约测试：JsonPatch 消息包含 `seq`，且在 entity-level patch 场景包含 `invalidate` 且符合 spec 形状（`cargo test -p logs-axum`）

## 2. Frontend：WS JsonPatch 应用降分配与降渲染

- [x] 2.1 为 id-map patch（`/tasks/<id>` 等）实现结构共享 fast-path，并集成到 `frontend/src/hooks/useJsonPatchWsStream.ts`（复杂 patch fallback 到 RFC6902 路径），补对照测试（`pnpm -C frontend run test`）
- [x] 2.2 在 `useJsonPatchWsStream` 中引入 patch 批处理（rAF/microtask flush），确保顺序一致且 reconnect/resync 行为不回归（`pnpm -C frontend run test`）
- [x] 2.3 将 WS `invalidate` 作为 hint 暴露给消费侧（例如可选回调/状态），为 tasks hooks 的局部派生重算提供输入（`pnpm -C frontend run test`）

## 3. Frontend：tasks 派生增量化与页面重复分组清理

- [x] 3.1 重构 `useAllTasks` / `useProjectTasks`：避免每条 patch 全量 `Object.values + sort + group`，改为基于 changed ids 的局部更新/引用稳定输出，并补一致性测试（对照旧实现结果）（`pnpm -C frontend run test`）
- [x] 3.2 清理页面层重复分组/排序：`TasksOverview` / `ProjectTasks` 等只做一次可 memo 的派生，避免 render 内循环重建结构（`pnpm -C frontend run check` + 手动 smoke）

## 4. 验收与回归

- [x] 4.1 跑后端全量回归：`cargo test --workspace` + `pnpm run backend:check`（或 `just qa` 覆盖）
- [x] 4.2 跑前端回归：`pnpm -C frontend run check` + `pnpm -C frontend run lint` + `pnpm -C frontend run test`
- [x] 4.3 跑整仓质量门禁：`just qa`
