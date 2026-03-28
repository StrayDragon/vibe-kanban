## Why

当前 realtime WS JSON-Patch 链路在任务量/更新频率较高时会出现明显的 CPU/内存峰值：前端对每条 patch 做 `structuredClone` 并全量重建派生结构（排序/分桶/分组），后端在部分热路径上也存在不必要的 DB fan-out（例如单任务更新扫描整项目任务列表）。这会导致 UI 卡顿、频繁 GC、以及后端负载放大。

## What Changes

- 后端：将 tasks 相关 patch 的生成改为“按需最小读取/最小写入”，避免单 task 更新时扫描整项目任务列表（减少 DB/CPU 放大）。
- 后端：明确并稳定 WebSocket JSON-Patch 消息中的 `invalidate` hints 形状与语义（保持 additive/backward-compatible），为前端增量派生提供可靠信号。
- 前端：为 id-map 型 patch（如 `/tasks/<id>`、`/projects/<id>`、`/execution_processes/<id>`）实现结构共享的快速应用路径，避免对整棵 state 做深拷贝；并对 patch 应用做批量合并（降低渲染次数与短期分配）。
- 前端：重构 tasks 派生（数组排序、按 status/project 分组）为增量/引用稳定的实现，避免每条 patch 触发全量 `Object.values + sort + group` 以及页面层重复分组。
- 测试与验收：补齐后端事件/stream 回归测试、前端增量派生一致性测试（与现有 RFC6902 语义对照），并新增针对 `invalidate` hints 的契约测试。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `realtime-stream-resilience`: 细化 `invalidate` hints 的 JSON schema、语义与兼容策略，确保前端无需解析 JSON Pointer 也能进行缓存失效与局部重算。

## Impact

- Backend: `crates/events/`, `crates/db/`, `crates/logs-axum/`, `crates/server/`（tasks stream / event emission / WS message envelope）
- Frontend: `frontend/src/hooks/useJsonPatchWsStream.ts`, tasks hooks（`useAllTasks`/`useProjectTasks`/archived variants），以及任务列表页面派生逻辑
- 风险：实时数据一致性（patch 应用语义）、优化引入的局部更新遗漏、以及 stream 恢复/lag resync 行为回归

