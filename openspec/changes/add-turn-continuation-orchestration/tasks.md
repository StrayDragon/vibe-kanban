## 1. Continuation policy, overrides, and persistence

- [ ] 1.1 Add project-level default continuation budget (default-off) with migrations + API wiring. Verification: `cargo test -p db -p server && pnpm run generate-types`
- [ ] 1.2 Add task-level continuation budget override (inherit/disable/override) with precedence logic. Verification: `cargo test -p db -p server && pnpm run generate-types`
- [ ] 1.3 Add `task_orchestration_states` (new table) to persist continuation counters + structured stop reasons, designed to be reusable by MCP collaboration follow-ups. Verification: `cargo test -p db && cargo test --workspace`

## 2. Same-session continuation execution

- [ ] 2.1 Implement continuation decision and follow-up start in the execution finalization path (successful coding-agent turn), before the normal `inreview` handoff. Ensure human queued follow-up messages are executed first. Verification: `cargo test -p execution && cargo test --workspace`
- [ ] 2.2 Add a short continuation prompt builder that composes: latest turn summary + remaining budget + “resume without restating base prompt” guidance (do not re-send full orchestration prompt). Verification: `cargo test -p server && cargo test -p execution`
- [ ] 2.3 Define and implement a parseable completion marker (for example `VK_NEXT: continue|review`) and default missing marker to stop. Map outcomes into structured stop reasons. Verification: `cargo test --workspace`

## 3. Auto-only diagnostics and UX boundaries

- [ ] 3.1 Expose continuation counters, effective budgets, and stop reasons only in auto-managed task detail, task lists, and MCP-readable task surfaces (manual surfaces unchanged). Verification: `cargo test -p server && pnpm run generate-types`
- [ ] 3.2 Ensure manual task UI and behavior remain unchanged when continuation support exists in the codebase. Verification: `pnpm run check && pnpm run lint`

## 4. End-to-end validation

- [ ] 4.1 Smoke-test an auto-managed task that continues for one additional turn in the same session and then stops at a review handoff boundary. Verification: `cargo test --workspace` plus one manual browser/MCP smoke check
