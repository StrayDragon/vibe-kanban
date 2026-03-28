## Why

当前日志链路（`logs-protocol` / `logs-store`）在热路径上多次通过 `serde_json::to_string(...).len()` 做“字节预算”估算，并且 `MsgStore` 同时维护多组大容量 broadcast buffer（默认 10k）导致：
- 额外的 CPU 与堆分配（尤其 `JsonPatch` 大小估算与 entry-json 大小估算）。
- 高并发/高吞吐输出时内存占用不稳定，且容易出现锁竞争与 backlog。

为了在 server 常驻场景下做到更低 CPU、稳定低内存占用，需要把日志预算与广播 buffer 做到“按需、可配置、低分配”。

## What Changes

- 用“无堆分配”的 JSON 长度估算替代热路径的 `serde_json::to_string(...).len()`：
  - `LogMsg::JsonPatch` 的 `approx_bytes()` 不再序列化 patch 计算长度。
  - `logs-store` 对 raw/normalized entry 的 `Value` 大小估算不再序列化成字符串计算长度。
- `MsgStore` 的 broadcast buffer 容量可配置，并下调默认值；在发生 lagged 时进行可恢复的 resync（不再静默丢失）。
- 减少 `MsgStore::push` 的锁内工作（把 patch 解析/提取尽量移到锁外），降低锁竞争与尾延迟。
- 代码层面移除冗余通道/冗余拷贝（按需统一到 sequenced 流，减少重复保存相同 payload 的概率）。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `execution-logs`: 增加对“日志预算/广播 buffer 必须低分配且可配置”的非功能性约束；并明确 lagged 情况下的 resync 行为（避免静默丢失）。

## Goals
- 显著降低 `JsonPatch` 与 entry 预算计算带来的 CPU/alloc。
- 降低 `MsgStore` 常驻内存占用与峰值（尤其 broadcast ring buffer）。
- 在高吞吐输出下保持日志流可恢复（lagged 时 resync，而不是悄悄缺失）。

## Non-goals
- 不改变对外 HTTP/SSE/WS payload 的语义与字段结构（除非 spec 明确要求）。
- 不在本变更中重做日志持久化/DB schema（只做内存链路与流式传输的改进）。

## Risks
- JSON 长度估算如果低估会导致预算失效（内存超出预期）；如果高估会导致更早 eviction（可观察行为变化）。
- broadcast capacity 下调可能增加 lagged 频率；需要确保 resync 路径正确并且不会导致重复处理/重复规范化。

## Verification
- 新增单元测试覆盖：
  - JSON 长度估算对典型 payload 的正确性（至少不低于真实 `serde_json::to_string` 长度）。
  - lagged/resync 场景下流不丢失或按预期恢复。
- 运行：`cargo test --workspace`、`just qa`、`just openspec-check`。
