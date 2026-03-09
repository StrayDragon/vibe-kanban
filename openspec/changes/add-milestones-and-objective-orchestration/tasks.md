## 1. TaskGroup-as-Milestone Persistence + Types

- [ ] 1.1 Add DB migration to extend `task_groups` with milestone columns: `objective`, `definition_of_done`, `default_executor_profile_id`, `automation_mode` (default `manual`). Verification: `pnpm run prepare-db && cargo test -p db`
- [ ] 1.2 Update SeaORM entity + Rust model DTOs (`TaskGroup`, `CreateTaskGroup`, `UpdateTaskGroup`) to round-trip milestone fields, trimming empty strings to `None`. Verification: `cargo test -p db`
- [ ] 1.3 Regenerate shared TypeScript types after DTO changes. Verification: `pnpm run generate-types`

## 2. API + Prompt Injection

- [ ] 2.1 Extend `/api/task-groups` create/update routes to accept and persist milestone fields and return them on reads. Verification: `cargo test -p server`
- [ ] 2.2 Extend TaskGroup node prompt augmentation to inject milestone objective/definition-of-done (when present) in addition to node instructions. Verification: `cargo test -p execution`
- [ ] 2.3 Add a focused API action to advance a milestone by one step (for example “run next eligible node”), reusing the existing attempt start flow and returning a structured result describing what was started or why nothing was eligible. Verification: `cargo test -p server`

## 3. Scheduler Eligibility for Milestone-Managed Grouped Work

- [ ] 3.1 Introduce a backend “milestone eligibility” resolver for grouped tasks (node ready, no other in-progress node attempts in the same group, automation enabled). Verification: `cargo test -p server`
- [ ] 3.2 Update auto-orchestration candidacy/diagnostics so grouped node tasks are dispatchable only when milestone automation is enabled, and otherwise report an explicit reason. Verification: `cargo test --workspace`
- [ ] 3.3 Add regression tests proving the scheduler dispatches at most one node attempt per milestone at a time. Verification: `cargo test --workspace`

## 4. Frontend UX (Milestone Creation, Editing, Progress)

- [ ] 4.1 Rename user-facing “Task Group” terminology to “Milestone” in creation flows, badges, and workflow page copy (keep internal API naming stable). Verification: `pnpm run check && pnpm run lint`
- [ ] 4.2 Add milestone metadata editing to the workflow view (master/entry node panel): objective, definition-of-done, default executor profile, automation toggle, and progress summary. Verification: `pnpm run check && browser smoke`
- [ ] 4.3 Add a “Run next step” control wired to the one-step API action, with clear disabled states and diagnostics (blocked by deps, checkpoint gate, already running). Verification: `pnpm run check && browser smoke`

## 5. End-to-End Validation

- [ ] 5.1 Run full suite checks. Verification: `cargo test --workspace && pnpm run check && pnpm run lint`
- [ ] 5.2 Manual browser validation with DevTools:
  - create a milestone, set objective/DoD, add 2-3 nodes with a dependency and a checkpoint
  - enable milestone automation and observe exactly one node dispatch at a time
  - verify network payloads and task dispatch transitions are coherent
  Verification: `pnpm run dev` + DevTools Network/Console smoke
