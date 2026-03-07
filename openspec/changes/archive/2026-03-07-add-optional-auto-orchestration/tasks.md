## 1. Shipped phase-1 foundation

- [x] 1.1 Add project-level execution mode, scheduler limits, and database migration for optional auto orchestration. Verification: `cargo check -p db -p repos -p server`
- [x] 1.2 Add persisted task dispatch state and include automation metadata in task list/detail DTOs. Verification: `cargo check -p db -p tasks -p server && pnpm run generate-types`
- [x] 1.3 Add the background auto orchestrator loop that reuses the existing task-attempt creation flow with bounded retry backoff. Verification: `cargo test -p server auto_orchestrator::tests::retry_backoff_is_capped`
- [x] 1.4 Surface manual vs auto project state and task automation badges/details in the frontend. Verification: `pnpm run check && pnpm run frontend:lint`

## 2. Task-level control surfaces

- [x] 2.1 Add `inherit | manual | auto` task automation override to the task data model, generated types, and persistence layer. Verification: `cargo check -p db -p tasks -p server && pnpm run generate-types && pnpm run check`
- [x] 2.2 Extend existing project/task mutation surfaces so both interactive users and MCP/programmatic callers can toggle automation settings without bespoke orchestration endpoints. Verification: `cargo test --workspace && pnpm run check`
- [x] 2.3 Add a clearer switch-based UX for project/task automation states so users can understand `manual`, `inherit`, and `auto-managed` at a glance. Verification: `pnpm run check && pnpm run frontend:lint`

## 3. Scheduling diagnostics

- [x] 3.1 Add structured "why not scheduled" diagnostics to task list/detail responses, including stable reason codes and human-readable detail. Verification: `cargo check -p db -p tasks -p server && cargo test -p server`
- [x] 3.2 Render diagnostics in task list/detail UI so skipped, blocked, deferred, and review-waiting tasks are understandable without log inspection. Verification: `pnpm run check && pnpm run frontend:lint`
- [x] 3.3 Add regression coverage for manual default behavior, task override precedence, grouped-task exclusion, retry gating, and review handoff. Verification: `cargo test -p server && cargo test -p repos`

## 4. Workflow prompt adaptation

- [x] 4.1 Extract the most relevant unattended-run prompt patterns from `../symphony/elixir/WORKFLOW.md` and land a `vk`-native orchestration prompt template document at `docs/auto-orchestration-prompt.md` without Linear/GitHub-specific instructions. Verification: `docs/auto-orchestration-prompt.md` exists and passes team review against the design checklist.
- [x] 4.2 Wire auto-managed task dispatch to render the versioned workflow prompt with task/project/repository/attempt context. Verification: `cargo check -p tasks -p server && cargo test -p server`
- [x] 4.3 Add tests or prompt fixtures covering first-run vs retry/continuation rendering and blocker/handoff instructions. Verification: `cargo test -p server`

## 5. Final smoke checks

- [x] 5.1 Smoke-test manual project, auto project, task-level override flows, and the adapted unattended prompt end to end, including one grouped task that stays unscheduled with a visible reason. Verification: `pnpm run check && cargo test --workspace`

## 6. Human-first orchestration UX and policy follow-up

- [x] 6.1 Add an orchestration overview strip plus `Manual` / `Managed` / `Needs Review` / `Blocked` task filters to the main task surfaces. Verification: `pnpm run check && pnpm run frontend:lint`
- [x] 6.2 Add a task-detail ownership banner and handoff summary card so auto-managed results are reviewable without log-diving. Verification: `pnpm run check && pnpm run frontend:lint`
- [x] 6.3 Reuse existing attempt/session/diff metadata to power a human review inbox and clear approve/rework/takeover actions before introducing new persistence. Verification: `cargo test -p server && pnpm run check`
- [x] 6.4 Add task lineage/source metadata for agent-created related follow-up tasks and surface linked-task context in API, MCP, and UI. Verification: `cargo check -p db -p server && pnpm run generate-types && cargo test --workspace`
- [x] 6.5 Add explicit policy/documentation for MCP-driven automation: support task-level auto requests, keep project-level auto changes explicit, and cover the rules with tests. Verification: `cargo test -p server && cargo test --workspace`

## 7. Future follow-up candidates (non-blocking)

These items came out of late review and MCP/agent discussion, but they do not block shipping or archiving this change because the human-first optional auto-orchestration scope above is now implemented and validated.

- MCP-readable review handoff payloads that bundle latest summary, diff summary, and validation status for review-ready auto-managed tasks.
- Explicit persisted control-transfer reasons for pause / take-over / resume between human and auto-managed states.
- Project policy for allowed executor/profile variants for auto-managed work, plus structured diagnostics when an MCP caller requests a disallowed profile.
- Event-friendly orchestration transition surfaces so MCP callers can react to claim / retry / blocked / review transitions without tight polling.
- A separate design spike for remaining Symphony-adjacent gaps such as external tracker bridges, richer ops dashboards, and stale-claim/workspace recovery semantics.

