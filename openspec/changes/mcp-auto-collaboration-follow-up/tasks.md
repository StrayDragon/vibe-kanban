## 1. Control-transfer and policy foundations

- [ ] 1.1 Reuse the existing `task_orchestration_states` record and add explicit write points for control-transfer reasons (at minimum `awaiting_human_review` and `policy_rejected_profile`; add `human_takeover` / `human_resume` when the corresponding control surfaces exist). Expose these diagnostics in shared task DTOs, scoped to auto-managed tasks only. Verification: `cargo test -p db -p server && pnpm run generate-types`
- [ ] 1.2 Add project-scoped executor+variant allow-list policy for MCP-driven auto-managed work, stored as project DB settings with conservative default `inherit_all`. Enforce only at MCP entry points when callers explicitly request overrides and only for auto-managed tasks. Persist policy rejection diagnostics so later reads can explain "why". Verification: `cargo test -p db -p server && cargo test --workspace`

## 2. MCP review handoff contracts

- [ ] 2.1 Add a focused MCP read tool for review-ready handoff payloads keyed by `task_id` (optionally accept `attempt_id`) with explicit output schema coverage. Non-managed / non-review tasks should return a structured "not applicable / unavailable" result rather than failing. Verification: `cargo test -p server mcp::task_server::tools`
- [ ] 2.2 Enrich existing MCP task/feed reads with transfer and policy diagnostics without breaking current schemas. Diagnostics should remain scoped to auto-managed tasks so non-managed tasks do not accidentally surface automation-only fields. Verification: `cargo test -p server && cargo test --workspace`

## 3. Human-surface parity

- [ ] 3.1 Mirror the new transfer/policy reasons in existing task detail and review UI surfaces without creating a separate MCP-only workflow. Keep UI scoped to auto-managed tasks to avoid confusing operators on normal tasks. Verification: `pnpm run check && pnpm run lint`
- [ ] 3.2 Publish orchestration transition events through existing feed surfaces and ensure they match the persisted reason model. Keep the event set minimal (review-ready, takeover/resume, policy rejection) to avoid feed noise. Verification: `cargo test -p server && cargo test --workspace`

## 4. End-to-end validation

- [ ] 4.1 Smoke-test a review-ready auto-managed task where an MCP client reads handoff state and chooses approve, rework, or take-over without raw log scraping. Verification: `cargo test --workspace` plus one manual MCP smoke check
