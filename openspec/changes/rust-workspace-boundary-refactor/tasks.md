## 1. Baseline (already implemented)

- [x] 1.1 Introduce `crates/executors-protocol` and move persisted/shared executor types into it (verify: `cargo tree -p db | rg \"\\bexecutors v\"` returns nothing)
- [x] 1.2 Update `db` to depend on `executors-protocol` (not `executors`) and make `ExecutionProcess.executor_action` strict after migration (verify: `cargo test --workspace`)
- [x] 1.3 Add DB migration that upgrades legacy `executor_action` JSON to the strict protocol schema (verify: `pnpm run prepare-db:check`)
- [x] 1.4 Ensure remote DB schema checks pass (verify: `pnpm run remote:prepare-db:check`)

## 2. Crate boundary guardrails (CI)

- [x] 2.1 Add a boundary-check script (e.g. `scripts/check-crate-boundaries.sh`) that asserts: protocol crates do not depend on Axum/rmcp; core crates (e.g. `db`, `services`) do not depend on Axum/rmcp; `db` does not depend on `executors` runtime (verify: run the script locally; it exits 0)
- [x] 2.2 Wire boundary checks into CI (GitHub Actions) alongside existing `cargo clippy`/`cargo test` checks (verify: CI job runs the script)

## 3. Logs: protocol/store split (remove Axum from `utils`)

- [x] 3.1 Create `crates/logs-protocol` containing transport-agnostic log message types currently in `crates/utils/src/log_msg.rs` (verify: `cargo tree -p logs-protocol | rg axum` returns nothing)
- [x] 3.2 Create `crates/logs-store` containing `MsgStore` and history/stream logic currently in `crates/utils/src/msg_store.rs` (verify: `cargo tree -p logs-store | rg axum` returns nothing)
- [x] 3.3 Create `crates/logs-axum` providing Axum-only adapters (`LogMsg` → SSE/WS, SSE stream helpers) used by `server`/`deployment` (verify: `cargo check -p logs-axum`)
- [x] 3.4 Update all call sites to use the new crates; remove `utils::log_msg` and `utils::msg_store` exports (verify: `cargo clippy --all --all-targets -- -D warnings`)
- [x] 3.5 Remove `axum` from `crates/utils/Cargo.toml` and ensure `db` no longer pulls in Axum transitively (verify: `cargo tree -p db | rg \"\\baxum v\"` returns nothing)
- [x] 3.6 Remove direct `axum` deps from non-adapter crates that no longer need them (at minimum `crates/services/Cargo.toml` and `crates/executors/Cargo.toml`) (verify: `cargo tree -p services | rg \"\\baxum v\"` returns nothing and `cargo tree -p executors | rg \"\\baxum v\"` returns nothing)

## 4. Decompose `utils` into focused crates

- [x] 4.1 Create `crates/utils-core` and move lightweight, broadly used helpers out of `utils` (verify: `cargo check -p utils-core`)
- [x] 4.2 Create `crates/utils-assets` and move `rust-embed`/asset helpers out of `utils` (verify: `cargo check -p utils-assets`)
- [x] 4.3 Create `crates/utils-git` and move git2-based helpers out of `utils` (verify: `cargo check -p utils-git`)
- [x] 4.4 Create `crates/utils-jwt` and move jsonwebtoken helpers out of `utils` (verify: `cargo check -p utils-jwt`)
- [x] 4.5 Update workspace crates to depend on the smallest required utils crate(s) and remove unused heavy deps from core crates (verify: `cargo tree -p db | rg \"\\bgit2 v|\\breqwest v|\\bjsonwebtoken v\"` returns nothing)
- [x] 4.6 Remove or shrink the old `crates/utils` crate so it no longer acts as a catch-all; update all imports in one sweep (no compatibility re-exports) (verify: `cargo test --workspace`)

## 5. Modularize `executors` for replaceable providers

- [x] 5.1 Create `crates/executors-core` and move shared executor runtime logic out of `crates/executors` (verify: `cargo check -p executors-core`)
- [x] 5.2 Create provider crates (e.g. `executor-codex`, `executor-claude`, `executor-cursor`, `executor-gemini`, etc.) and move provider implementations (verify: `cargo check -p executor-codex` and at least one other provider crate)
- [x] 5.3 Convert `crates/executors` into a small facade that wires enabled providers behind Cargo features and exposes the registry used by `services/server` (verify: `cargo check -p executors` with default features)
- [x] 5.4 Ensure `db` and protocol crates remain independent of executor runtime/provider crates (verify: `cargo tree -p db | rg \"\\bexecutor-\"` returns nothing)

## 6. Decompose `server` MCP task server implementation

- [x] 6.1 Split `crates/server/src/mcp/task_server.rs` into smaller modules (params/errors/runtime/tools) without changing tool semantics (verify: `cargo test -p server`)
- [x] 6.2 Build the MCP binary and run a basic Inspector smoke test against it (verify: `cargo build -p server --bin mcp_task_server` and `just mcp-inspector`)

## 7. Workspace-wide verification + TS types

- [x] 7.1 Run Rust workspace checks after each phase (verify: `cargo fmt --all` + `cargo clippy --all --all-targets -- -D warnings` + `cargo test --workspace`)
- [x] 7.2 Regenerate TypeScript types after protocol type moves (verify: `pnpm run generate-types`)
