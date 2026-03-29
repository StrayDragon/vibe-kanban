## 1. Lag resync 状态机去双份物化

- [x] 1.1 重构 `MsgStore::raw_history_plus_stream()` 的 lag resync：用 `snapshot + cursor` 状态逐条产出 Replace，移除 `pending VecDeque` 的全量复制（验证：`cargo test -p logs-store raw_stream_resyncs_after_lag_and_continues`）
- [x] 1.2 重构 `MsgStore::normalized_history_plus_stream()` 的 lag resync：保持语义一致并避免 snapshot 双份缓冲（验证：`cargo test -p logs-store normalized_stream_resyncs_after_lag_and_continues`）

## 2. 测试与验收

- [x] 2.1 如有需要补充/更新测试以覆盖 lag resync 与 Finished 时序（验证：`cargo test -p logs-store`）
- [x] 2.2 运行并修复直到通过：`just qa`、`just openspec-check`

## 3. 归档与提交

- [x] 3.1 归档该 change（`openspec archive -y c2-msgstore-resync-snapshot-slimdown`）并创建最终 commit：`refactor: msgstore-resync-snapshot-slimdown`
