## 0. Implementation discipline

- [x] 0.1 Land one numbered phase at a time; do not start the next phase until the current phase passes its acceptance gate.
- [x] 0.3 Do not keep compatibility re-exports, duplicate facades, or dual wiring after a phase is complete; update all internal call sites in that phase.

## 1. Guardrails and crate scaffolding

- [x] 1.1 Add empty `crates/repos`, `crates/tasks`, `crates/execution`, `crates/config`, `crates/events`, and `crates/app-runtime` crates to the workspace so later phases can migrate into stable names.
- [x] 1.2 Update `scripts/check-crate-boundaries.sh` for `repos` / `tasks` / `execution` / `config` / `events`, and add the source-level import checks defined in the design doc.
- [x] 1.3 Run the phase-1 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh`.

## 2. Extract low-risk leaf capabilities: `config` and `events`

- [x] 2.1 Move `services/config/*` and `services/cache_budget.rs` into `config` and update all direct consumers in one pass.
- [x] 2.2 Move `services/events.rs` and `services/events/*` into `events` and update all direct consumers in one pass.
- [x] 2.3 Remove the old `services` exports for config and event code in the same phase.
- [x] 2.4 Run the phase-2 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh`.

## 3. Extract `repos`

- [x] 3.1 Move `project`, `repo`, `git/*`, `worktree_manager`, `filesystem`, `filesystem_watcher`, `file_search_cache`, `file_ranker`, and `workspace_manager` into `repos`.
- [x] 3.2 Keep blocking git/filesystem internals inside `repos` and expose async-safe APIs only.
- [x] 3.3 Rewire `projects`, `repo`, and `filesystem` route groups plus any MCP/repo helpers to use `repos`.
- [x] 3.4 Remove the old `services` exports for repo/filesystem code in the same phase.
- [x] 3.5 Run the phase-3 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh`.

## 4. Extract `execution`

- [x] 4.1 Move `services/container/mod.rs`, `diff_stream.rs`, `queued_message.rs`, `image.rs`, `github.rs`, `github/cli.rs`, and `pr_monitor.rs` into `execution`.
- [x] 4.2 Move `local-deployment/src/container.rs`, `command.rs`, and `copy.rs` into `execution` in the same phase.
- [x] 4.3 Rewire execution-oriented routes, session routes, and runtime/MCP helpers that only need execution/container behavior.
- [x] 4.4 Remove the old `services` exports and local-deployment execution helpers in the same phase.
- [x] 4.5 Run the phase-4 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh && cargo build -p server --bin mcp_task_server`.
- [x] 4.6 Smoke-test one execution flow before continuing: start backend, create or select a task attempt, and verify logs / execution-process endpoints still work.

## 5. Extract `tasks`

- [x] 5.1 Move `services/approvals.rs` and `services/approvals/executor_approvals.rs` into `tasks`.
- [x] 5.2 Move task/attempt orchestration, archived-kanban coordination, and idempotent create/start flows into `tasks`.
- [x] 5.3 Rewire task-centric routes, approval flows, and MCP handlers to call `tasks`.
- [x] 5.4 Remove the old `services` exports for task orchestration in the same phase.
- [x] 5.5 Run the phase-5 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh && cargo build -p server --bin mcp_task_server`.
- [x] 5.6 Smoke-test one task lifecycle before continuing: create task, start attempt, approve or deny an approval, and verify the task/attempt state stays correct.

## 6. Introduce `app-runtime` and retire composition facades

- [x] 6.1 Move startup sequencing, shutdown wiring, `notification`, and background job registration from `deployment` / `local-deployment` into `app-runtime`.
- [x] 6.2 Rewire `server` startup to consume `app-runtime` and narrow capability entrypoints instead of a broad deployment façade.
- [x] 6.3 Remove `crates/deployment` and the remaining composition responsibilities from `crates/local-deployment` in the same phase.
- [x] 6.4 Run the phase-6 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh && cargo build -p server --bin mcp_task_server`.
- [x] 6.5 Smoke-test backend startup and one end-to-end path after the composition swap before continuing.

## 7. Slim `executors`

- [x] 7.1 Reduce `crates/executors` to registry/facade responsibilities only.
- [x] 7.2 Move any remaining shared provider/runtime helpers into `executors-core`.
- [x] 7.3 Verify every `executor-*` crate stays independent of `server`, `app-runtime`, and all capability crates.
- [x] 7.4 Run the phase-7 acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh && cargo build -p server --bin mcp_task_server`.

## 8. Final transport cleanup and legacy crate removal

- [x] 8.1 Rewire any remaining `server` route groups and MCP modules to capability entrypoints only.
- [x] 8.2 Split remaining oversized transport modules after ownership is stable.
- [x] 8.3 Delete `crates/services` only after every moved area is already served by `repos`, `tasks`, `execution`, `config`, `events`, or `app-runtime`.
- [x] 8.4 Run the final acceptance gate: `cargo check --workspace && cargo test --workspace && pnpm run backend:check && ./scripts/check-crate-boundaries.sh && cargo build -p server --bin mcp_task_server`.
- [x] 8.5 Smoke-test the final core paths before closing the change: list projects/repos, create task, start attempt, tail logs/feed, and complete one approval flow.
