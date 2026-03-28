## Why

当前 `logs-store` 的 `MsgStore` 在 lag/resync 与 WS 广播路径中存在大量 `LogMsg`/JSON payload 的 clone 与整段 history materialize，容易放大短期内存分配与 CPU 开销，并在高频日志/多订阅者场景下导致 RSS 峰值与尾延迟上升。

## What Changes

- 后端：`MsgStore` 内部改为结构共享（例如 `Arc`/raw JSON buffer 复用），避免 per-subscriber/per-resync 的重复分配与 clone。
- 后端：lag/resync 的 snapshot 输出改为分块/分页式推送（或等价的 bounded 构造），避免一次性构造超大 JSON 树。
- 后端：补齐针对 lag/resync、history paging、以及序列化形状的回归测试，确保对外协议与语义不变。
- 指标：补充关键观测（例如 resync 次数、resync snapshot bytes、encode 耗时），用于验证优化效果。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `execution-logs`: 强化 lag/resync 与 in-memory history 的性能/内存约束（不改变对外消息形状与语义）。

## Impact

- Rust：`crates/logs-store/`（核心）、`crates/logs-axum/`（WS 输出/序列化边界）、以及相关 server 路由/服务调用点
- Verification: `cargo test -p logs-store -p logs-axum`, `cargo test --workspace`, `just qa`, `just openspec-check`

