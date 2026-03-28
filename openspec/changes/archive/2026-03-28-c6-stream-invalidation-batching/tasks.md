## 1. Backend：SSE invalidation 去重

- [x] 1.1 调整 `crates/app-runtime/src/lib.rs` 的 `stream_events`：当 `SequencedLogMsg` 可生成 `invalidate` hints 时，只发送 `invalidate` event（同 `seq` 不再额外发送 `json_patch`）；当 hints 不可用时，继续发送 `json_patch` fallback。
- [x] 1.2 增加 `app-runtime`/`server` 单测：验证 hints 可用时同一 `seq` 只出现一个 SSE event；hints 不可用时仍发送 `json_patch`。
- [x] 1.3 增加/更新单测：锁定 `invalidate_all`（resume_unavailable / lagged）行为不变（包含 id=watermark 与 payload 字段）。

## 2. Frontend：invalidation batching + 去重

- [x] 2.1 实现一个 invalidation batcher（合并 `taskIds`/`workspaceIds`/`hasExecutionProcess`），并在 `frontend/src/contexts/EventStreamContext.tsx` 中使用它来处理 `invalidate` 与 `json_patch` fallback（短窗口 flush，最小化 `invalidateQueries` 调用次数）。
- [x] 2.2 更新/新增前端单测：覆盖“多个 hints 在一个 batch 内合并并去重”的行为，并保持现有 invalidation 语义（对应 queryKey 集合）不变。

## 3. Verification

- [x] 3.1 Run `cargo test -p app-runtime -p server`.
- [x] 3.2 Run `pnpm -C frontend test` and `pnpm -C frontend run lint`.
- [x] 3.3 Run `just qa` and `just openspec-check`.
