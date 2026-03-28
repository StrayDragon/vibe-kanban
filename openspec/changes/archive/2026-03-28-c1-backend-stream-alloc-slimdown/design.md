## Context

当前 backend realtime streams（`crates/events/src/streams.rs`）在构造 snapshot patch 时使用 `serde_json::json!` 生成 `Value`，再通过 `serde_json::from_value` 反序列化回 `json_patch::Patch`。此外，stream 过滤逻辑为了判断是否需要转发，会对 `PatchOperation::{Add,Replace}` 的 `value` 做 `clone + from_value::<T>`，引入多余的 clone 与全量反序列化开销。

在 `crates/logs-axum/src/lib.rs` 中，`SequencedLogMsg::to_ws_message_unchecked()` 通过 `to_value -> mutate -> to_string` 的方式拼接 `seq` 与 `invalidate` hints，也会导致每条 WS 消息额外的中间 `Value` 树分配；而 invalidation hints 解析通过 `split_pointer_path()` 对每条 op 分配 `Vec<String>`，在高频 patch 下会放大 allocator 压力。

本变更目标是在**不改变 wire shape/语义**的前提下，把上述热点路径改为“最小字段解析 + 最少分配”的实现。

## Goals / Non-Goals

**Goals:**
- 让 tasks/projects/execution_processes 的 snapshot patch 直接构造 `json_patch::Patch`（避免 `Value` 往返）。
- 让 tasks/execution_processes 的 patch 过滤只解析必要字段（避免 `Value` clone 与整对象反序列化）。
- 让 WS envelope 输出避免 `to_value` 中间树；同时让 invalidation hints 的 JSON Pointer 解析避免分配 `Vec<String>`。
- 增加回归测试，锁定消息形状与过滤语义，避免性能优化引入协议/一致性回归。

**Non-Goals:**
- 不改变任何 API 路由、消息字段命名、或 JSON Patch 语义（仅内部实现优化）。
- 不引入新的外部依赖、不调整 DB schema、不重做 outbox/MsgStore 架构。
- 不做前端优化（前端另起 change 处理）。

## Decisions

1) **Snapshot patch 直接构造 `Patch`**
- 方案：使用 `json_patch::{Patch, PatchOperation, ReplaceOperation}`，直接构建 `Patch(vec![ReplaceOperation{path:"/tasks", value:Object(tasks_map)}])`。
- 原因：减少一次 `Value` 构造 + 一次 `from_value` 反序列化；逻辑更直接；失败点更少。
- 备选：继续沿用 `json!`/`from_value`（简单但多余往返/分配）。

2) **过滤逻辑改为“最小字段解析”**
- 方案：对 `PatchOperation::{Add,Replace}` 的 `value: &Value` 直接读取 `project_id`/`archived_kanban_id`/`session_id`/`dropped` 等字段；仅在字段缺失/类型不符时 fallback 为“保守处理”（例如不转发或按 remove 处理）。
- 原因：`TaskWithAttemptStatus`/`ExecutionProcessPublic` 体积较大，频繁 `from_value` 会产生显著 CPU/alloc 压力；过滤所需字段很少。
- 备选：保留 `from_value::<T>`（实现简单但 clone 与反序列化成本高）。

3) **WS envelope 直接序列化**
- 方案：为 WS 输出构造一个轻量的可序列化结构（按 variant 分支填充），一次 `serde_json::to_string` 输出最终 JSON。
- 原因：避免 `to_value -> mutate -> to_string` 的中间 `Value` 树与哈希表扩容。
- 备选：继续使用 `to_value`（简单但分配多）。

4) **invalidation hints 的 JSON Pointer 解析“只取前两段”**
- 方案：只解析 path 的 root segment（tasks/workspaces/execution_processes）与可选 id segment；仅对 id segment 做必要的 `~0/~1` decode；避免构造 `Vec<String>`。
- 原因：hints 只需要前两段；完整 split 会产生额外分配。
- 备选：维持 `split_pointer_path`（实现简单但高频分配）。

## Risks / Trade-offs

- [过滤逻辑的边界行为变化] → 对缺失字段/非预期 payload 采用保守策略，并用单测覆盖（确保不会把不该转发的 patch 误转发）。
- [WS JSON shape 细微差异] → 通过 golden-ish 结构断言测试锁定字段存在性与类型（尤其是 `seq`、`invalidate`、以及 legacy variant 字段）。
- [微优化引入复杂度] → 控制在局部 helper 函数内，避免跨模块重构；保持代码可读与可回滚。

