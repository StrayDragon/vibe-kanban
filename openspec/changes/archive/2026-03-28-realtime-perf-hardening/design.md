## Context

当前 realtime WS JSON-Patch 链路存在几个已确认的热路径，导致 CPU/内存占用放大并在高更新频率下出现卡顿：

- **Frontend**
  - `frontend/src/hooks/useJsonPatchWsStream.ts`：每条 JsonPatch 消息都对 `current` 做 `structuredClone()`，再用 `rfc6902.applyPatch()` 变更（热路径深拷贝 + 全量分配）。
  - `frontend/src/hooks/tasks/useAllTasks.ts` / `frontend/src/hooks/projects/useProjectTasks.ts`：每次 state 变化都会 `Object.values + sort + 按 status 分桶`；页面层（如 `frontend/src/pages/TasksOverview/TasksOverview.tsx`）又重复做分组与统计，导致 **单条 patch 触发多次全量 O(n log n)**。
  - WS 消息类型已包含 `invalidate?: unknown`，但当前未消费，无法将重算范围限制在“受影响实体集合”。

- **Backend**
  - `crates/events/src/lib.rs`：`emit_task_patch()` 在非 remove 场景下会先拉取某项目下所有任务（`Task::find_by_project_id_with_attempt_status`）再从中筛选单个 task，这是典型 **O(n) 扫描**，并且内部存在 **DB fan-out**。
  - `crates/db/src/models/task/mod.rs`：`find_filtered_with_attempt_status()` 对每个 task 调 `with_attempt_status()`，里面包含多次查询（attempt_status / dispatch_state / orchestration），当需要 snapshot 或 lag-resync 时会放大。
  - `crates/logs-axum/src/lib.rs`：对 `LogMsg::JsonPatch` 的 invalidation hints 生成目前经由 `serde_json::to_value()` + JSON Pointer 扫描，存在不必要的 `Value` 往返与重复解析。

约束：
- 协议上保持 additive/backward-compatible（字段新增不破坏 legacy 客户端）。
- 不修改 `shared/types.ts`（如需变更需从 Rust types 生成）。
- 目标是可回滚、可验证、增量落地（避免大规模架构改写）。

## Goals / Non-Goals

**Goals:**
- 显著降低 tasks/projects/workspaces/execution_processes realtime 更新链路的短期分配与 GC 压力。
- 将高频更新下的 UI 重渲染范围收敛到“受影响实体/列表片段”，而非每条 patch 全量重建。
- 消除后端 task 更新发 patch 时的“扫描整项目任务列表”行为，降低 DB/CPU 放大。
- 明确 `invalidate` hints 的 shape/语义，并补齐契约测试。

**Non-Goals:**
- 不引入新的网络协议（如二进制帧/Protobuf）。
- 不重写为全局状态管理架构（例如把所有 stream state 迁到单一 store/reducer）。
- 不在本 change 内全面重构 logs/对话渲染性能（可作为后续 change）。

## Decisions

### 1) Backend：`emit_task_patch` 改为按需最小读取

**现状**
- `crates/events/src/lib.rs:238` 会先 `find_by_project_id_with_attempt_status(project_id)` 拉全量任务再筛单个 id。

**决策**
- 将 add/replace 场景改为直接 `Task::find_by_id_with_attempt_status(db, task_id)` 获取单条 `TaskWithAttemptStatus`，然后生成 patch。
- `project_id` 仅作为事件 payload 的冗余信息保留，不再用于拉全量任务列表。

**原因**
- 这是链路上最确定的后端 P0 放大点：单实体更新不应触发 O(n) 扫描。

**替代方案**
- 保持扫描以“对齐 active task list view 的 automation diagnostics”。结论：成本过高；应通过单条读取保证一致性，并用回归测试确保字段一致。

### 2) Backend：`invalidate` hints 生成从 `Value` 扫描改为 typed patch 扫描

**现状**
- `crates/logs-axum/src/lib.rs` 为 JsonPatch 消息插入 `invalidate` 时，需要先把 patch 转成 `serde_json::Value` 再解析 JSON Pointer。

**决策**
- 实现 `invalidation_hints_from_patch(patch: &json_patch::Patch)`，直接遍历 `PatchOperation`，解析 `op.path()` 并在必要时读取 `op.value`（例如 workspaces 的 `task_id`），避免 `to_value()` 往返。
- 仍保持输出为 `invalidate` JSON object，兼容既有 `WsJsonPatchMsg.invalidate?: unknown`。

**替代方案**
- 在 patch 生成侧（`crates/events/src/patches.rs`）直接附带 typed hints。结论：需要扩展协议 envelope（跨 crates/logs-protocol），本 change 不做。

### 3) Frontend：JsonPatch 应用引入“id-map 快速路径 + fallback”

**现状**
- `frontend/src/hooks/useJsonPatchWsStream.ts` 对所有 patch 都 `structuredClone(current)`。

**决策**
- 在 `useJsonPatchWsStream` 内实现 fast-path：
  - 若 patch 仅包含 `replace /tasks`（snapshot）或对 `/tasks/<id>` 的 add/replace/remove（以及同形态的 `/projects/<id>`、`/workspaces/<id>`、`/execution_processes/<id>`），则只 shallow clone 受影响的 map（结构共享）。
  - 遇到无法识别/不安全的 patch（例如深层字段 patch、混合路径）时，fallback 到现有 `structuredClone + applyPatch`（保证语义正确）。

**原因**
- tasks stream 的 patch 形态在后端固定为“整实体替换到 `/tasks/<id>`”，是最典型、最可优化的 hot path。

**替代方案**
- 彻底移除 fallback，强制所有 patch 符合 id-map 形态。结论：当前协议与未来扩展风险较高；应先通过 fast-path 覆盖热点，再用测试逐步收敛 patch 形态。

### 4) Frontend：批量合并 patch 应用以减少 render 次数

**决策**
- 在 WS onmessage 中将 patch 写入队列，使用 `requestAnimationFrame`（或 microtask）批量 flush：一次 flush 中对当前 state 连续应用多条 patch，最终只 `setData` 一次。

**原因**
- 高频 patch 下“每条消息一次 setState”会导致渲染风暴；批量合并可显著降低 CPU。

### 5) Frontend：tasks 派生计算从“全量重建”收敛到“局部重算”

**决策（分两步）**
1. 先把 `invalidate` hints 与 patch-path 解析结合起来，得到 `changedTaskIds` 集合，减少无关派生重算。
2. 在 `useAllTasks` / `useProjectTasks` 内部引入增量索引（按 status/project 的有序 id 列表），仅对变更 task 做局部插入/移除/移动；并移除页面层重复分组逻辑。

**替代方案**
- 仅做 memo/减少重复遍历。结论：收益有限，任务量大时仍会每条 patch O(n log n)。

## Risks / Trade-offs

- [Risk] fast-path patch 解析遗漏路径或语义不一致 → Mitigation：引入“随机 patch 对照测试”（fast-path 结果必须与 RFC6902 fallback 完全一致），并对异常 patch 强制走 fallback。
- [Risk] patch 批处理引入轻微 UI 延迟 → Mitigation：使用 rAF（<=16ms），并在 finished/resync/错误路径立即 flush。
- [Risk] 后端改为按 id 读取后，字段与旧路径不一致 → Mitigation：为 `emit_task_patch` 增加回归测试，覆盖 archived/non-archived、含 orchestration/dispatch_state 的场景。
- [Risk] invalidation hints 逻辑从 `Value` 扫描改为 typed 扫描后漏掉提示 → Mitigation：为 tasks/workspaces/execution_processes 的 add/replace/remove 全覆盖单测，包含 `~0`/`~1` 转义路径。

## Migration Plan

- 无需用户数据迁移；变更为纯运行时优化与协议字段的稳定化（additive）。
- 发布策略：合并后通过 `just qa` 与关键 e2e/前端单测；如发现回归可直接 revert 单个 commit 回滚。

## Open Questions

1. **Snapshot patch 是否需要 `invalidate` 的“全量”语义？**
   - 推荐：snapshot 只发 `replace /tasks` 等全量替换，不附带巨大 `taskIds` 列表；前端将 snapshot 视为“全量失效”并重建派生。
2. **前端应优先使用 `invalidate` 还是解析 patch path？**
   - 推荐：以 patch-path 作为 correctness 的依据（能准确知道更新了哪些实体），`invalidate` 作为加速 hint（减少解析/减少额外查询与派生重算）。
3. **DB fan-out 的 batch 优化是否纳入本 change？**
   - 推荐：本 change 至少完成 `emit_task_patch` 的 O(1) 改造；是否继续做 `find_filtered_with_attempt_status` 的 batch/join 优化取决于 profiling（若 lag-resync/snapshot 仍占主导，再在同 change 或后续 change 落地）。

