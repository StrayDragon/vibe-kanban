## 1. MsgStore 结构共享

- [x] 1.1 将 `logs-store` 的 stored history / broadcast payload 改为结构共享（例如 `Arc<...>`），避免 `LogMsg` 深拷贝；修复所有调用点编译通过。
- [x] 1.2 将 `subscribe_sequenced_from()`/history snapshot 与 lag/resync replay 改为“只复制轻量引用”，并确保 replay 仅包含 `seq > last_seq`。
- [x] 1.3 将 raw/normalized entries 的存储与分页/广播事件改为共享表示，避免 `serde_json::Value` 的重复 clone（保持对外 JSON shape 不变）。

## 2. 观测与验收口径

- [x] 2.1 为 lag/resync 与 history eviction 增加必要的 tracing 字段（replayed 条数、snapshot bytes、encode 耗时等）以支持量化验证。

## 3. Tests

- [x] 3.1 增加 `logs-store` 单测：lag/resync replay 的最小性（`seq > last_seq`）、顺序一致性、以及 beyond retained window 的显式降级语义不变。
- [x] 3.2 增加 `logs-store` 单测：history budgets 淘汰与 resync 交互（evicted 标记、min/max seq 元数据）保持一致。
- [x] 3.3 如 `logs-axum`/WS 输出路径因类型调整而受影响，补齐回归测试锁定 WS payload shape（包含 `seq`/`invalidate`）不变。

## 4. Verification

- [x] 4.1 Run `cargo test -p logs-store -p logs-axum`.
- [x] 4.2 Run `cargo test --workspace`, `just qa`, and `just openspec-check`.
