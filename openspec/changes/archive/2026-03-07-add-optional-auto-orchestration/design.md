## Context

`vk` now has a shipped phase-1 implementation of optional project-level auto orchestration. The current foundation adds:

- project execution mode and scheduler settings
- persisted task dispatch state
- a background scheduler loop in `server`
- task DTO enrichment for automation status
- basic project/task UI badges and settings controls

The implementation is intentionally conservative: manual remains the default, only internal tasks are orchestrated, grouped tasks are skipped, and there is not yet a per-task override or a first-class diagnostic panel for skipped work.

This design backfills the architecture for the shipped foundation and defines the next phase so the system can support a clearer switch model for both humans and programmatic callers.

## Goals

- Keep automation optional and additive to the existing manual workflow.
- Reuse the current task-attempt runtime path.
- Add finer-grained control without introducing a separate orchestration subsystem.
- Explain scheduler behavior clearly enough that operators do not need to inspect logs to understand why a task did or did not run.

## Non-goals

- External work intake from third-party issue trackers.
- Dependency-aware scheduling for task groups in this change.
- Replacing current attempt/session/execution-process tables or executor selection flow.
- Building proof-bundle or quality-gate subsystems in this change.

## Architecture Overview

### 1. Shipped phase-1 foundation

#### Project controls

Projects persist three automation fields:

- `execution_mode: manual | auto`
- `scheduler_max_concurrent: i32`
- `scheduler_max_retries: i32`

These fields are enough to turn the scheduler on per project while keeping the current manual experience as the default.

#### Dispatch state

A dedicated `TaskDispatchState` record stores scheduler-visible state per task:

- controller (`manual | scheduler`)
- dispatch status (`idle | claimed | running | retry_scheduled | awaiting_human_review | blocked`)
- retry counters and retry timestamps
- blocked/error details

Persisting this state avoids overloading task status and gives the UI a stable source for automation badges.

#### Scheduler loop

The background loop polls auto-enabled projects, reconciles dispatch state, counts active attempts, and dispatches eligible tasks through the existing orchestration entrypoint.

Eligibility is deliberately narrow in phase 1:

- only internal `vk` tasks
- no in-progress attempts
- not done/cancelled
- grouped tasks excluded
- retries gated by `next_retry_at`

#### Existing UI

The current UI shows:

- project-level manual vs auto mode
- scheduler settings in project settings
- task-level automation badges and basic details (`retry scheduled`, `blocked`, `awaiting review`)

### 2. Next-phase task-level control

Project-level control is useful, but it is too coarse for mixed projects. The next phase adds a task field:

- `task_automation_mode: inherit | manual | auto`

Semantics:

- `inherit`: follow project execution mode
- `manual`: never auto-dispatch this task
- `auto`: allow scheduler dispatch even if the project default is manual

This lets a mostly-manual project opt a few tasks into automation, and lets an auto project keep exceptions under human control.

### 3. Effective automation decision

The scheduler should evaluate an effective mode per task:

1. start from project `execution_mode`
2. apply task override if present
3. reject unsupported shapes such as task-group entry tasks and grouped child tasks
4. reject retry-blocked or review-blocked tasks
5. enforce concurrency limits

This keeps project mode as the default while allowing task-level exceptions without a second scheduler.

### 4. Diagnostics model

The current implementation exposes dispatch state, but it does not explain every non-run case. The next phase should add a computed diagnostic payload to task list/detail responses, for example:

- `reason_code`
- `reason_detail`
- `actionable`

Representative reason codes:

- `project_manual`
- `task_manual_override`
- `task_auto_override`
- `task_group_unsupported`
- `retry_not_ready`
- `retry_exhausted`
- `awaiting_human_review`
- `concurrency_limit_reached`
- `no_project_repos`
- `base_branch_unresolved`

This payload should be read-only and derived from current project/task/runtime state rather than persisted unless it is already part of `TaskDispatchState` (for example `blocked_reason`).

### 5. UX and programmatic control surfaces

#### Interactive UX

The system should expose a switch-like control model:

- project settings: manual vs auto orchestration
- task detail/list: inherit/manual/auto-managed control with clear copy
- tooltips or a detail panel for "why not scheduled"

The UX should make it obvious that automation is optional and reversible.

#### Programmatic UX

The same automation fields should be writable through existing mutation surfaces instead of creating a special-purpose mega API:

- project update surface for project execution mode and scheduler settings
- task update surface for task automation override
- MCP tools should use the same fields so automation can be enabled by agents or external controllers without custom side channels

### 6. Prompt adaptation from Symphony

The next phase should borrow the strongest unattended-session prompt patterns from `../symphony/elixir/WORKFLOW.md` while removing tracker-specific assumptions.

#### Prompt behaviors worth reusing

- unattended-session posture
- continuation/retry instructions using the current attempt number
- validation-first execution before declaring completion
- strict blocker policy instead of casual human escalation
- concise final summaries focused on completed work and blockers

#### Required `vk` adaptations

The prompt must be `vk`-native rather than Linear-native:

- use task/project/repository/workspace context instead of issue tracker fields
- reference `vk` review handoff and task status, not Linear state transitions
- reference existing `vk` attempt/session/log surfaces instead of an external workpad comment
- allow explicit human review handoff inside `vk`, while still forbidding vague “user follow-up” requests during unattended execution

#### Prompt contract

The system should keep a repo-versioned template for auto orchestration and render it with structured inputs such as:

- task identifier, title, description, and current status
- project name and automation mode
- repository/workspace context
- attempt number for continuation/retry runs
- any relevant validation or acceptance instructions already attached to the task

Prompt rendering should be treated as part of orchestration correctness. If the template cannot render with the required context, the dispatch should fail visibly and surface a scheduler reason rather than starting a malformed run.

### 7. Rollout plan

#### Phase 1 — already shipped

- project-level auto/manual mode
- scheduler concurrency/retry settings
- dispatch-state persistence
- scheduler loop and attempt dispatch
- project/task status badges

#### Phase 2 — follow-up in this change

- task-level override (`inherit/manual/auto`)
- better switch UX for users and MCP callers
- structured "why not scheduled" diagnostics
- a `vk`-native orchestration prompt adapted from Symphony workflow guidance
- regression coverage for override precedence and unsupported grouped tasks

## Risks

- Task-level `auto` override on a manual project can surprise users if the UI copy is weak.
- Diagnostics can become misleading if eligibility and rendering logic drift apart.
- Adding mutation fields to task/programmatic surfaces requires careful shared-type regeneration and compatibility checks.

## Verification

- Backend: `cargo check -p db -p repos -p tasks -p app-runtime -p server`
- Targeted tests: scheduler retry/backoff and override precedence tests
- Shared types: `pnpm run generate-types`
- Frontend: `pnpm run check && pnpm run frontend:lint`
- Smoke checks:
  - project manual ↔ auto toggle
  - task inherit/manual/auto override behavior
  - grouped task remains unscheduled with a visible reason
  - blocked/retry/review reasons are visible without inspecting backend logs

## Update after shipped foundation

The implementation described in the earlier sections has now landed more completely than the original draft anticipated:

- task-level `inherit | manual | auto` override is shipped
- structured automation diagnostics are shipped
- the `vk`-native auto-orchestration prompt template is shipped

The next design increment is therefore no longer about basic control surfaces. It is about making automation legible to humans, making auto-managed outcomes easier to consume, and safely supporting agent-created related tasks.

## 7. Human-first orchestration UX

### UX principles

- `vk` remains a human-first task product; automation is an execution mode, not a separate product.
- Ownership must be explicit: manual, auto-managed, and waiting-for-human-review should look different at first glance.
- Use icon + label + color, not color alone.
- The primary action must change based on ownership state so users do not accidentally fight the scheduler.
- Automation diagnostics should be visible inline and expandable, not buried in logs.

### Visual language

| Surface | Manual | Auto-managed | Waiting Human Review | Blocked / Deferred |
|--------|--------|--------------|----------------------|--------------------|
| Owner chip | `User` icon + `Manual` | `Bot` icon + `Auto-managed` | `Eye` icon + `Needs review` | `AlertTriangle` icon + reason |
| Accent | neutral/slate | indigo/blue | amber | red/orange |
| Primary CTA | `Start attempt` | `Take over manually` or `Run once now` | `Review result` | `Inspect reason` |
| Secondary CTA | `Enable auto` | `Pause automation` | `Request rework` / `Approve` | `Retry when ready` |

Recommended style direction from `ui-ux-pro-max`:

- product pattern: analytics / operations dashboard
- style: minimal + dark mode / high-contrast status surfaces
- components: `Switch`, `Tabs`, `Badge`, `Table`, `Drawer`, `Alert`, `Tooltip`
- interaction rules: visible focus states, no color-only state, loading feedback for async toggles, responsive table fallback on mobile

### Information architecture

#### Project-level control center

Add a compact orchestration summary strip above task lists:

- `Execution Mode` switch with explicit copy: `Manual` / `Auto-managed`
- counts for `Running`, `Retry Queue`, `Needs Review`, `Blocked`
- optional filter chips / tabs: `All`, `Manual`, `Managed`, `Needs Review`, `Blocked`

This mirrors the spirit of Symphony's `status_dashboard` (`Running`, `Backoff queue`, status snapshot) but keeps the task list as the primary human surface instead of introducing a separate ops-only console.

#### Task row / card

Each task row should show two distinct state groups:

- **ownership chip**: who owns execution policy right now
- **runtime chip**: running / retry scheduled / blocked / awaiting review / idle

Below the title, show one short diagnostic line when relevant:

- `Manual project by default`
- `Task opted into automation`
- `Waiting for retry window`
- `Blocked: no project repos configured`

#### Task detail banner

At the top of task detail, add a persistent ownership banner:

- current owner (`Manual`, `Auto-managed`, `Needs human review`)
- why this mode applies (project default, task override, blocked reason)
- mode-appropriate actions

Example actions:

- manual task: `Start attempt`, `Enable auto for this task`
- auto task: `Take over manually`, `Pause automation`, `Open diagnostics`
- awaiting review: `Review result`, `Approve`, `Request rework`, `Keep auto after rework`

#### Diagnostics drawer

Instead of only a tooltip, add a details drawer/panel with:

- effective automation mode
- project mode vs task override
- scheduler reason code + human detail
- latest retry timestamp / next retry timestamp
- missing prerequisites (repos, base branch, grouped-task restriction)

## 8. Human result consumption and handoff

Symphony uses one persistent workpad comment plus a dedicated `Human Review` state so humans know where to look. `vk` does not have that exact tracker/comment model, so the equivalent should be task-centric rather than tracker-centric.

### Proposed `vk` handoff model

Use a single handoff card in task detail sourced from existing runtime data:

- latest attempt summary
- diff summary (`files changed`, `added`, `deleted`)
- validation summary / failure summary when available
- PR / approval / review-comment signal when available
- latest automation diagnostic if the run ended blocked or deferred

This gives humans one place to inspect result quality before deciding what to do next.

### Decision model for humans

This can be implemented mostly with existing data and status fields:

- `Approve result` -> mark task `done`, clear dispatch to `idle`
- `Request rework` -> move task back to active work (`todo` or `inprogress`), preserving automation mode
- `Take over manually` -> set task override to `manual`
- `Resume auto` -> keep `inherit` or `auto`, return task to schedulable state

### Data impact assessment for handoff

No new table is required for the first version if the handoff card is read from existing sources:

- `task.status`
- `task.automation_mode`
- `task.dispatch_state`
- latest attempt/session summary and diff summary
- pending approvals / PR comments when available

A new persistence layer is only needed if we later want stable historical handoff snapshots independent of attempt retention. If that becomes necessary, add a lightweight `task_handoffs` table or a denormalized `latest_handoff_summary` read model.

## 9. Agent-created related tasks (子 task / follow-up task)

The current data model is already good enough for an agent or MCP caller to create another normal task and mark it `automation_mode=auto`. However, that is not yet human-friendly because the new task has no first-class lineage back to the task that spawned it.

### Recommendation

Keep the first step small:

- add `origin_task_id: Option<Uuid>` or a small task-relations table
- add `created_by_kind: human_ui | mcp | scheduler | agent_followup`
- expose both in task list/detail/MCP responses
- show a linked-task module in the UI: `Created from Task X` / `Spawned follow-up tasks`

This is enough to support Symphony-style “discovered follow-up work should become a separate tracked item” without requiring a full dependency graph redesign.

### Scheduling policy for related tasks

Agent-created related tasks should not always auto-run. Add a project policy layer:

- `manual_only`: agent-created tasks are forced to `inherit`/manual behavior
- `inherit_project`: agent-created tasks may inherit the project mode
- `allow_auto`: agent-created tasks may explicitly request `automation_mode=auto`

That keeps human trust high while still enabling a future “auto handles its own follow-up tasks” path.

## 10. MCP and external agent policy

### Current state

Today, MCP callers can already:

- create tasks with `automation_mode`
- update tasks with `automation_mode`
- inspect `project_execution_mode`, `effective_automation_mode`, and automation diagnostics

Because the scheduler scans all projects, another agent can already create a task with `automation_mode=auto` and have it auto-run even when the project default is manual.

### What MCP cannot safely do yet

MCP does **not** currently expose project-level mutation for:

- `execution_mode`
- `scheduler_max_concurrent`
- `scheduler_max_retries`

This is a good default. Project-wide automation changes are a much higher-blast-radius action than marking one task as auto-managed.

### Recommended MCP policy

- keep task-level auto requests supported
- keep project-level auto enablement behind an explicit future project-mutation tool
- if/when project-level MCP mutation is added, require an explicit field and make it operator-visible in the UI/audit trail
- never silently upgrade a whole project to auto mode as a side effect of task creation

## 11. What can be done with the current data model vs what needs more data

| Need | Current structure enough? | Notes |
|------|---------------------------|-------|
| Distinguish manual vs auto in UI | Yes | Use existing project mode, task override, effective mode, dispatch state, diagnostics |
| Show why task was not scheduled | Yes | Already available via diagnostics + dispatch state |
| Human review inbox / result card | Mostly yes | Derive from attempt summary, diff summary, validation/failure info |
| Let MCP agent request auto for a task | Yes | Already supported via task create/update with `automation_mode` |
| Let MCP agent enable project-wide auto | No | Needs explicit project-mutation surface and policy |
| Let agent create human-readable related tasks | Not cleanly | Recommend adding lineage/source fields before scaling this pattern |

## 9. MCP / agent collaboration design

### 9.1 Principles

- Human-first remains the product default; automation augments the task model instead of replacing it.
- MCP callers must use the same persisted task/project fields as the UI wherever possible.
- Control transfer must be explicit. If a human pauses or takes over a managed task, programmatic clients must see that immediately.
- Agent-created follow-up work must stay attributable and inspectable.
- Scheduler behavior must remain explainable from task state alone: effective mode, dispatch state, and a structured reason.

### 9.2 Recommended contract for MCP callers

The current mutation contract is a good foundation and should be extended carefully rather than replaced:

- task create/update keeps using `automation_mode`, `origin_task_id`, and `created_by_kind`
- reads must continue to expose `project_execution_mode`, `effective_automation_mode`, `dispatch_state`, and machine-readable diagnostics
- review/handoff reads should expose one concise summary surface composed from latest session summary + diff summary + validation status
- manual takeover / pause / resume actions should produce a visible state transition that both humans and MCP clients can observe without reading logs

### 9.3 Executor/profile safety

Symphony assumes a repo/workflow-owned execution environment. `vk` needs a safer project-centric rule set because MCP callers may have different preferences from the human operator.

Recommended policy:

- project policy owns the default executor/profile for auto-managed work
- MCP callers may optionally request a specific executor/profile only when that profile is allowed by project policy
- if the request is not allowed, `vk` should preserve the task but surface a clear diagnostic instead of silently downgrading or silently escalating
- the selected effective executor/profile should be inspectable in attempt/session surfaces

### 9.4 Eventing and legibility

Polling alone is enough for the first shipped version, but agent callers will cooperate better with explicit transition events:

- task became eligible
- task claimed by scheduler
- retry scheduled
- blocked with actionable reason
- awaiting human review
- resumed manually or resumed into automation

These can ride existing event/tail surfaces first; a separate orchestration-only protocol is not required.

### 9.5 Remaining Symphony gaps worth copying later

- tracker bridge adapter for external ticket pulling/writing
- richer ops dashboard for retry queue and orchestration health
- stall recovery / stale claim reclamation / cleanup policy
- stronger PR / review loop choreography for unattended flows

