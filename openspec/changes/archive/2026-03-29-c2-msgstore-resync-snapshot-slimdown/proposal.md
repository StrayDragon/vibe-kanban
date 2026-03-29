## Why

`MsgStore::{raw,normalized}_history_plus_stream()` 在 `broadcast::RecvError::Lagged` 场景下会对 retained window 做全量 snapshot，并把 snapshot 中每条 entry 再转换为 `Replace` 事件塞进 `pending` 队列。

即使 history 本身已有 `VK_LOG_HISTORY_MAX_BYTES/ENTRIES` 上限，该 resync 逻辑仍会造成明显的瞬时资源放大：

- 同一份 snapshot 同时以 `Vec<LogEntrySnapshot>` + `VecDeque<LogEntryEvent>` 双份存在；
- lag 频繁发生时会重复 snapshot + rebuild 队列，带来 CPU/alloc 抖动；
- subscriber 数越多，resync 的重复成本越高。

## What Changes

- 将 lag resync 的实现改为“按需流式重放 snapshot”：保留一次 snapshot（或其可迭代视图），逐条产出 `Replace` 事件，避免构建第二份 `pending` 队列副本。
- 保持现有语义不变：Append/Replace 事件形态、entry_index 单调性、Finished 行为不变；仅优化内部内存占用与锁持有/分配。
- 增加/更新测试覆盖：确保 lag 后仍能 resync 并继续流式输出（raw 与 normalized 两条路径都覆盖）。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `execution-logs`: 增加性能护栏——lag resync 的实现应避免对 retained snapshot 做额外副本放大（保持语义一致）。

## Impact

- Backend: `crates/logs-store/src/msg_store.rs`（lag resync 状态机、pending 结构）
- Tests: `crates/logs-store/src/msg_store.rs`（resync 相关 tokio 测试）

## Goals

- 显著降低 lag resync 的峰值内存占用与分配次数。
- lag 发生时仍保持正确性与最终一致：resync 后继续输出新事件直到 Finished。

## Non-goals

- 不改变 retained history 的预算/驱逐策略（bytes/entries 上限保持）。
- 不改变客户端协议与事件格式（仍使用 `LogEntryEvent::{Append,Replace,Finished}`）。
- 不引入新的存储后端或持久化日志。

## Risks

- resync 状态机改动可能引入边界 bug（例如 Finished 的时序、Replace 顺序）→ 通过单测覆盖 raw/normalized 两条路径并在 `just qa` 中回归。

## Verification

- 单测：`cargo test -p logs-store`
- 全量：`just qa`、`just openspec-check`
