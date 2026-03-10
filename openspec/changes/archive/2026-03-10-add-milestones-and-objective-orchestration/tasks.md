## 1. Hard Cut: Remove Legacy Task/Project Auto Orchestration

- [x] 1.1 Add DB migration to remove `projects.execution_mode` and `tasks.automation_mode` (hard cut). Verification: `pnpm run prepare-db && cargo test -p db`
- [x] 1.2 Delete automation-only DTO fields from task reads (`project_execution_mode`, `effective_automation_mode`, `automation_diagnostic`) and remove the associated reason-code enums/types. Verification: `cargo test --workspace && pnpm run generate-types:check`
- [x] 1.3 Remove legacy automation write surfaces:
  - HTTP: task create/update no longer accepts `automation_mode`; project update no longer accepts `execution_mode`.
  - MCP: task tools no longer accept or return `automation_mode`/`effective_automation_mode`/`project_execution_mode`.
  Verification: `cargo test -p server mcp::task_server::tools`
- [x] 1.4 Remove legacy automation UI:
  - Project Settings no longer shows “Execution Mode”.
  - Task creation/edit no longer shows automation mode picker.
  - Remove orchestration lanes/filters keyed to task automation mode.
  Verification: `pnpm run check && pnpm run lint`

## 2. Milestone (TaskGroup) Persistence + Types

- [x] 2.1 Add DB migration to extend `task_groups` with milestone columns: `objective`, `definition_of_done`, `default_executor_profile_id`, `automation_mode` (default `manual`), plus `run_next_step_requested_at`. Verification: `pnpm run prepare-db && cargo test -p db`
- [x] 2.2 Update SeaORM entity + Rust model DTOs (`TaskGroup`, `CreateTaskGroup`, `UpdateTaskGroup`) to round-trip milestone fields, trimming empty strings to `None`. Verification: `cargo test -p db && pnpm run generate-types:check`

## 3. API + Prompt Injection

- [x] 3.1 Extend `/api/task-groups` create/update routes to accept and persist milestone fields and return them on reads. Verification: `cargo test -p server`
- [x] 3.2 Extend TaskGroup node prompt augmentation to inject milestone objective/definition-of-done (when present) in addition to node instructions. Verification: `cargo test -p execution`
- [x] 3.3 Add a focused API action to enqueue “run next step” for a milestone, returning a structured response describing what was enqueued (or why nothing was eligible). Verification: `cargo test -p server`

## 4. Scheduler: Milestone-Only Dispatch

- [x] 4.1 Replace the scheduler dispatch loop to select next eligible milestone node tasks (and never regular tasks). Verification: `cargo test --workspace`
- [x] 4.2 Implement milestone one-at-a-time guarantees:
  - at most one in-progress node attempt per milestone
  - project-level concurrency limits still apply
  Verification: `cargo test --workspace`
- [x] 4.3 Consume `run_next_step_requested_at` as a durable enqueue:
  - prioritize pending run-next-step requests
  - clear the request once an attempt is started (or return a stable non-eligible reason at enqueue time)
  Verification: `cargo test --workspace`

## 5. Frontend UX (Milestone Creation, Editing, Progress)

- [x] 5.1 Rename user-facing “Task Group” terminology to “Milestone” in creation flows, badges, and workflow page copy. Verification: `pnpm run check && pnpm run lint`
- [x] 5.2 Add milestone metadata editing to the workflow view (master/entry node panel): objective, definition-of-done, default executor profile, automation toggle, progress summary, and “Run next step”. Verification: `pnpm run check`

## 6. End-to-End Validation

- [x] 6.1 Run full suite checks via just. Verification: `just check && cargo test --workspace`
- [x] 6.2 Manual browser validation with DevTools:
  - create a milestone, set objective/DoD, add 2-3 nodes with a dependency and a checkpoint
  - enable milestone automation and observe exactly one node dispatch at a time
  - use “Run next step” and confirm it enqueues (network request) and is consumed by the scheduler
  - verify Network payloads and task dispatch transitions are coherent
  Verification: `pnpm run e2e:test -- e2e/milestone-automation.spec.ts`
