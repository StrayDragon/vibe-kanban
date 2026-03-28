## 1. Snapshot Patch Slimdown

- [x] 1.1 Refactor `crates/events/src/streams.rs` tasks/projects/execution_processes snapshot builders to construct `json_patch::Patch` directly (no `json!` + `from_value`).
- [x] 1.2 Refactor `crates/server/src/routes/projects.rs` projects snapshot patch builder to construct `json_patch::Patch` directly.

## 2. Patch Filtering Without Full Deserialization

- [x] 2.1 Update tasks stream patch filtering to match `project_id` / `archived_kanban_id` via minimal `serde_json::Value` inspection (no `clone + from_value::<TaskWithAttemptStatus>`).
- [x] 2.2 Update execution processes stream patch filtering to match `session_id` / `dropped` via minimal `serde_json::Value` inspection (no `clone + from_value::<ExecutionProcessPublic>`).

## 3. WS Envelope + Invalidation Hints Alloc Slimdown

- [x] 3.1 Rewrite `crates/logs-axum/src/lib.rs` `SequencedLogMsg::to_ws_message_unchecked()` to serialize WS JSON without `to_value -> mutate -> to_string` while preserving legacy shapes.
- [x] 3.2 Optimize `crates/logs-axum/src/lib.rs` invalidation hints JSON Pointer parsing to avoid allocating `Vec<String>` per operation while preserving decode semantics.

## 4. Tests + Verification

- [x] 4.1 Add/adjust backend tests to lock down snapshot patch shapes and filtering semantics for tasks and execution_processes streams.
- [x] 4.2 Run `cargo test -p events -p logs-axum`, `cargo test --workspace`, `just qa`, and `just openspec-check`.
