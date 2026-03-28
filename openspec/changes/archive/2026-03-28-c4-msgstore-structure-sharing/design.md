## Context

`crates/logs-store/src/msg_store.rs` 为 `LogMsg` 提供了：
- 有界的 in-memory history（按 bytes 与 entries 限制淘汰）
- 基于 `tokio::broadcast` 的 live stream
- 当 receiver lagged 时的 resync（从 retained history 重放）

当前实现中，消息与历史快照在多个路径会被反复 clone / materialize：
- `push()` 会把同一条 `LogMsg` clone 到 history，同时再发送到 broadcast。
- 新订阅者初始 “history snapshot” 会把 retained history 复制为一个 `VecDeque` pending。
- lag/resync 会再次收集 `seq > last_seq` 的历史到临时 Vec，然后逐条 push 回 pending。
- raw/normalized entries 使用 `serde_json::Value` 存储，分页与广播事件会 clone `Value`。

在高频日志（stdout/stderr）、JsonPatch 更新密集、或多订阅者同时存在时，这些 clone 与临时集合会放大 CPU 与内存占用，并在 resync 时形成明显的短期峰值。

## Goals / Non-Goals

**Goals:**
- 在不改变对外协议与语义的前提下，显著减少 `LogMsg` 与 entry JSON 的重复分配与 clone。
- 保持 lag/resync 语义：在 retained window 内优先重放缺失区间；超出 window 仍可显式降级（由 `seq` 语义可检测 gap）。
- 保持 history 的 bytes/entries budget 约束，并让 resync 构造过程本身也保持 bounded（避免一次性构造超大临时 JSON 树）。
- 通过单测锁定 resync/replay 的最小性与顺序语义，并增加必要的观测字段用于验证效果。

**Non-Goals:**
- 不引入新的网络协议或二进制帧；不改变现有 WS payload 形状。
- 不改变 DB schema；不新增跨请求缓存或持久化策略。
- 不在本变更中重写前端日志渲染（前端优化另起 change 处理）。

## Decisions

1) **存储层引入结构共享**
- 方案：将 history 与 stream 内部持有的消息体改为共享引用（例如 `Arc<LogMsg>` / `Arc<Value>` / `Arc<RawValue>`），使：
  - history 与 broadcast 可共享同一份 message body
  - 新订阅者与 lag/resync 只复制轻量指针，而不是深拷贝内容
- 备选：继续用 owned `LogMsg`/`Value` 并在必要处做 clone（实现简单但热点成本高）。

2) **resync replay 保持“按需最小重放”**
- 方案：resync 时只重放 `seq > last_seq` 的 retained messages（保持现有语义），并明确不发送无必要的“全量 retained snapshot”。
- 备选：lag 直接发送全量 snapshot（实现简单但放大 IO 与内存，并对客户端造成不必要负担）。

3) **JSON entry 存储与输出避免重复构造**
- 方案：对 raw/normalized entries 采用共享存储，并在分页/广播时复用已构造的 JSON 结构（或 raw JSON buffer），避免 `entry_json.clone()`。
- 备选：继续 clone `Value`（在大 entry 或高频 replace 下开销明显）。

4) **观测与验收口径先行**
- 方案：在 resync/eviction/encode 热点处补充 tracing fields（如 resync 次数、replayed 条数、snapshot bytes、encode 耗时），并用基准测试/单测锁定语义与边界。
- 备选：仅靠肉眼判断与粗略 profiling（容易漏回归与无法量化收益）。

## Risks / Trade-offs

- [内部 API 变更波及面] → Mitigation：把结构共享封装在 `logs-store` 内部，对外暴露的 DTO 尽量保持稳定；必要的签名变更集中在 `logs-store`/`logs-axum` 边界处，并通过编译期修复全部调用点。
- [Arc/共享引用导致的生命周期/并发复杂度] → Mitigation：只做只读共享（消息体不可变），并保留现有 `RwLock<Inner>` 的一致性边界；避免引入跨线程可变共享结构。
- [预算统计不准确] → Mitigation：继续使用 `approx_json_value_len`/`approx_bytes()` 作为预算近似；对共享存储的 bytes 统计以“逻辑 bytes”计入（避免 budget 被误认为更小而失控）。

## Migration Plan

- 无需用户数据迁移；为纯运行时实现优化。
- 回滚策略：单个 commit 可 revert；对外协议不变，回滚风险低。
