## Why

当前后端 realtime streams（tasks/projects/execution_processes）在 snapshot / resume / lag resync 路径中存在多处不必要的 JSON `Value` 往返与反序列化：例如 snapshot 用 `json!([...])` 组装再 `from_value` 回到 `json_patch::Patch`，以及在过滤 patch 时对 `op.value` 做 `clone + from_value::<T>`。这在高频更新或 lag 恢复时会放大 CPU 与短期内存分配（进而触发更频繁的 GC/allocator 压力），并影响整体吞吐与尾延迟。

## What Changes

- 后端：tasks/projects/execution_processes 的 snapshot patch 直接构造 `json_patch::Patch`（避免 `json!` + `from_value` 往返）。
- 后端：stream 过滤/路由逻辑不再对 patch value 做整对象反序列化；改为只解析最小字段（如 `project_id` / `archived_kanban_id` / `session_id` / `dropped`）并避免 `Value` clone。
- 后端：WebSocket 输出消息的封装减少 `to_value -> mutate -> to_string` 的中间树分配；同时优化 invalidation hints 的 JSON Pointer 解析为“按需最小分段”（避免每条 op 分配 `Vec<String>`）。
- 测试：补齐后端序列化/协议形状回归测试，确保 WS payload（包含 `seq` / `invalidate`）与现有前端消费保持一致；补充过滤逻辑的单测覆盖边界输入。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `realtime-stream-resilience`: 在不改变现有消息形状的前提下，进一步约束服务端 patch 过滤与 envelope 输出的行为（例如只做最小字段解析），以保证在高频/lag 场景下的稳定性与可预测性。

## Impact

- Backend: `crates/events/src/streams.rs`, `crates/logs-axum/src/lib.rs`, `crates/server/src/routes/projects.rs`
- 风险：协议形状/过滤语义的细微偏差可能导致前端局部缓存失效或数据不同步；需要用测试与端到端 smoke 验证兜底
- Verification: `cargo test -p events -p logs-axum`, `cargo test --workspace`, `just qa`, `just openspec-check`

