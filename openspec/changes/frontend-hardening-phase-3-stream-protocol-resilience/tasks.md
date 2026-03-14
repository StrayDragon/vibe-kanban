## 1. Backend Sequencing Foundation

- [ ] 1.1 Add a monotonic `seq` assignment at the event/log message store layer while keeping existing unsequenced APIs intact (verify with Rust unit tests).
- [ ] 1.2 Expose a sequenced subscribe/history interface suitable for replay by `after_seq` (verify history eviction + min/max seq behavior).

## 2. WebSocket Resume (Entity Streams)

- [ ] 2.1 Extend WS stream endpoints to accept `after_seq` and to emit `seq` in an additive message envelope while preserving `JsonPatch`/`finished` fields (verify existing clients still connect).
- [ ] 2.2 Implement bounded replay: if `after_seq` is within retained history, replay missed messages; otherwise send a full snapshot (verify via targeted tests and local manual reconnect simulation).

## 3. Invalidation Hints

- [ ] 3.1 Generate backend invalidation hints from JSON Patch content for key domains (tasks/workspaces/execution processes) and attach them to WS messages (verify hint correctness in Rust tests).
- [ ] 3.2 Emit SSE event `id` equal to `seq` and add a new SSE event type carrying invalidation hints (verify SSE stream still works for legacy `json_patch` consumers).

## 4. Frontend Consumption & Fallback

- [ ] 4.1 Update WS stream consumers to track `seq`, pass `after_seq` on reconnect, and trigger resync on detected gaps (verify via `pnpm -C frontend run check` + `pnpm -C frontend run build`).
- [ ] 4.2 Prefer backend invalidation hints when present; fall back to existing client-side patch parsing when absent (verify by running with/without hints).

## 5. End-to-End Verification

- [ ] 5.1 Add/extend e2e tests that simulate WS disconnects/gaps and assert UI convergence (verify `pnpm run e2e:just-run`).
- [ ] 5.2 Run backend tests and checks: `cargo test --workspace` (or targeted crates) and ensure no regressions in streaming routes.
