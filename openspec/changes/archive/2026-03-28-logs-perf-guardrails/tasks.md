## 1. lagged/resync 可观测性（结构化日志 + 覆盖）

- [x] 1.1 调整 `crates/logs-store/src/msg_store.rs` 的 raw/normalized lagged 日志为结构化字段（至少包含 `skipped` 且明确 snapshot resync）
- [x] 1.2 为 `LogMsg` lagged（within-window / beyond-window）补齐日志断言测试（捕获 `tracing` 输出并验证包含关键字段）
- [x] 1.3 为 raw/normalized lagged snapshot resync 补齐日志断言测试（验证 `skipped` 字段与“snapshot resync”语义）

## 2. JSON 长度估算器性质测试（防低估回归）

- [x] 2.1 为 `crates/logs-protocol` 增加 `proptest` dev-dependency（仅测试使用）
- [x] 2.2 增加 `approx_json_value_len` 的 property-based 测试：随机 `serde_json::Value`（受控深度/大小）与 `serde_json::to_string(...).len()` 等价
- [x] 2.3 增加 `approx_json_patch_len` 的 property-based 测试：随机生成可解析 JSON Patch 并与 `serde_json::to_string(...).len()` 等价

## 3. 压力/慢消费者回归（不引入不稳定长测）

- [x] 3.1 增加 `LogMsg` 多订阅者 lagged/resync 回归测试（确保无重复/漏消息并最终 `Finished` 终止）
- [x] 3.2 增加 eviction + lag beyond retained window 回归测试（验证从 newest retained 继续且日志可诊断）
- [x] 3.3 增加 raw/normalized 多订阅者 lagged snapshot 回归测试（确保 Replace snapshot 后继续 Append）

## 4. 验证与归档

- [x] 4.1 运行并修复：`cargo test -p logs-protocol -p logs-store`
- [x] 4.2 运行并修复：`just qa`、`just openspec-check`
