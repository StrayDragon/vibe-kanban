## 1. Project hook configuration and persistence

- [x] 1.1 Add project-scoped `after_prepare` and `before_cleanup` hook configuration, validation guardrails, and config migration/defaulting. Verification: `cargo test -p server && cargo test --workspace`
- [x] 1.2 Persist latest hook outcome fields and expose them through shared DTOs without overloading existing setup-complete state. Verification: `cargo test -p server && pnpm run generate-types`

## 2. Workspace lifecycle execution

- [x] 2.1 Run `after_prepare` at the end of workspace preparation with correct `run_mode` and `failure_policy` behavior. Verification: `cargo test -p server && cargo test -p execution`
- [x] 2.2 Run `before_cleanup` during explicit remove-worktree and background cleanup flows with correct blocking vs warning semantics. Verification: `cargo test -p server && cargo test --workspace`

## 3. Existing surfaces and diagnostics

- [x] 3.1 Add project settings controls for lifecycle hooks and show latest hook outcome in existing workspace/task/attempt detail surfaces. Verification: `pnpm run check && pnpm run lint`
- [x] 3.2 Surface blocking hook failures as structured diagnostics for auto-managed dispatch and manual start flows without adding a new hook console. Verification: `cargo test -p server && cargo test --workspace`

## 4. End-to-end validation

- [x] 4.1 Smoke-test one project with a successful `after_prepare` hook and one with a warning-only `before_cleanup` hook. Verification: `cargo test --workspace` plus one manual browser smoke check
