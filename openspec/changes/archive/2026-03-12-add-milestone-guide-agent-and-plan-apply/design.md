## Context

VK milestones (implemented as the `milestones` table plus linked tasks/nodes/edges) enable goal-driven automation, but milestone authoring is still high-friction:

- Nodes require an existing `task_id`, so operators often have to create tasks first.
- Dependency edges are manual, but edges are required for:
  - sequential dispatch eligibility (predecessors must be `done`)
  - topology base strategy (derive base branches from completed predecessor workspaces)
- Iteration is expensive: changing the objective/DoD or restructuring the plan is a set of manual edits, not a guided workflow.

We want to make milestone planning conversational and low-friction without introducing a second orchestration engine or allowing unsafe background mutations.

## Goals / Non-Goals

**Goals:**
- Allow operators to plan milestones via conversation and apply the result as an executable graph (nodes + edges).
- Keep planning safe and auditable: validate, preview diff, then explicitly apply as a single deterministic mutation.
- Support iterative planning: re-run planning, compare diffs, and re-apply.
- Keep milestone dispatch semantics unchanged (one eligible node at a time; checkpoints gate downstream nodes).

**Non-Goals:**
- Unlimited autonomous planning/execution loops (continuation is handled separately).
- Auto-merge to main by default.
- A brand-new tracker-like orchestration runtime outside the existing scheduler + milestone dispatch.
- A “fully automatic planner” that writes tasks/edges without an explicit operator apply step.

## Decisions

### 1. Introduce a versioned Milestone Plan schema (separate from the graph schema)

Create a new, explicit payload contract (e.g. `MilestonePlanV1`) that a guide agent can emit and VK can validate. This avoids coupling planning output to the internal milestone graph JSON shape and gives us a stable migration story.

Plan v1 should support:
- milestone metadata updates: objective, definition of done, default executor profile, baseline ref, automation mode (all optional)
- nodes:
  - reference existing tasks by `task_id`
  - or create new tasks from `title`/`description`
  - node fields: kind, phase, base_strategy, requires_approval, instructions, executor_profile_id
- edges: `from`/`to` node ids, optional `data_flow`

### 2. “Preview then apply” is the only write path

The UI SHALL NOT directly mutate milestones in multiple ad-hoc calls (create task, then update milestone, then add edges). Instead, introduce:

- `POST /api/milestones/:id/plan/preview` -> validates and returns a diff summary
- `POST /api/milestones/:id/plan/apply` -> applies the plan atomically in one DB transaction

Rationale:
- avoids partial updates and scheduler races
- centralizes graph validity rules server-side
- supports stable error reporting and future MCP integration

### 3. Apply is idempotent and auditable

Plan application is a multi-entity mutation (tasks + milestone). We need both retry-safety and auditability:

- Use the existing HTTP idempotency mechanism (`Idempotency-Key`) for the apply endpoint.
- Persist a lightweight audit record (new table recommended) for:
  - plan schema version + normalized plan JSON
  - actor (human UI vs MCP vs scheduler) and optional executor identity
  - timestamps and resulting milestone revision info

New tasks created from a plan should use a distinct `created_by_kind` enum value (e.g. `milestone_planner`) for UX and audit.

### 4. The “Guide Agent” runs as a normal attempt/session on the milestone entry task

To keep architecture simple and reuse existing session history, the guide agent is represented as attempts on the milestone entry task (`task_kind=milestone`).

Key constraints:
- the guide workspace uses the normal per-attempt branch (not the milestone baseline ref), avoiding git worktree conflicts
- the milestone baseline ref remains the integration branch used as the base/target for node tasks

The guide agent prompt should instruct the agent to emit `MilestonePlanV1` in a strict JSON block so the UI can detect it reliably.

### 5. Deterministic “auto-wire topology” is a fallback, not the primary workflow

Provide an optional helper that can generate edges deterministically from node ordering (e.g. by phase, then left-to-right layout). This is useful when operators want a quick draft without invoking an agent, but it is not intended to replace explicit planning.

## Risks / Trade-offs

- [Plan parsing brittleness] → Use strict versioned schema + server-side validation; UI only recognizes fenced JSON with a sentinel header.
- [Partial apply corruption] → Single transactional apply endpoint; no multi-call mutation from the client.
- [Excessive scope creep into MCP] → Keep MCP tool additions optional; v1 can ship with UI-based preview/apply, then add MCP tools as follow-up.
- [Scheduler races] → Apply endpoint should preserve milestone automation mode by default and only change it when explicitly present in plan metadata.

## Migration Plan

- DB migration (if audit table is adopted): add `milestone_plan_applications` (or equivalent) with retention policy aligned with existing idempotency retention.
- Add new TS/Rust types for plan schema and preview/apply responses; regenerate types.
- Add new milestone workflow UI surfaces (Plan tab/panel) and e2e coverage.
- Roll out without changing dispatch logic; plan apply is additive.

## Open Questions

- Should v1 expose plan preview/apply through MCP tools so the guide agent can self-apply (still requiring explicit human approval), or keep apply as a UI-only action first?
- Should we persist plan drafts (not only applied plans), or keep drafts purely client-side until applied?

