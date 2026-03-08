## 1. Continuation policy, overrides, and persistence

- [ ] 1.1 Add project-level continuation policy with default-off behavior and migrations/defaulting for auto-managed orchestration settings. Verification: `cargo test -p server && pnpm run generate-types`
- [ ] 1.2 Add task-level continuation budget override (inherit/disable/override) and precedence logic. Verification: `cargo test -p server && pnpm run generate-types`
- [ ] 1.3 Persist continuation counters, timestamps, effective budgets, and structured stop reasons only for auto-managed tasks. Verification: `cargo test -p server && cargo test --workspace`

## 2. Same-session continuation execution

- [ ] 2.1 Add continuation eligibility evaluation that runs only after successful incomplete managed turns and remains distinct from retry-on-error logic. Verification: `cargo test -p server auto_orchestrator && cargo test --workspace`
- [ ] 2.2 Reuse existing follow-up/session infrastructure to queue continuation turns in the same workspace and session. Verification: `cargo test -p server && cargo test -p execution`
- [ ] 2.3 Implement a continuation prompt builder that derives a short follow-up prompt from the previous turn summary/result plus remaining budget (avoid re-sending a full fixed base prompt). Verification: `cargo test -p server auto_orchestrator_prompt && cargo test --workspace`

## 3. Auto-only diagnostics and UX boundaries

- [ ] 3.1 Expose continuation counters, effective budgets, and stop reasons only in auto-managed task detail, task lists, and MCP-readable task surfaces. Verification: `cargo test -p server && pnpm run generate-types`
- [ ] 3.2 Ensure manual task UI and behavior remain unchanged when continuation support exists in the codebase. Verification: `pnpm run check && pnpm run lint`

## 4. End-to-end validation

- [ ] 4.1 Smoke-test an auto-managed task that continues for one additional turn in the same session and then stops at a review handoff boundary. Verification: `cargo test --workspace` plus one manual browser/MCP smoke check
