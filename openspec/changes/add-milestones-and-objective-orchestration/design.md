## Context

Vibe Kanban already supports:
- atomic Tasks + Attempts + Sessions (with auto-managed execution via the scheduler)
- TaskGroups (workflow graphs) that group tasks and provide dependency-aware “ready vs blocked” semantics in the UI
- node-level prompting (`TaskGroupNode.instructions`) and executor profile preselection in the workflow view

What VK does not support today is “goal-driven” automation across a multi-step plan:
- there is no durable, inspectable objective (“definition of done”) scoped above individual tasks
- auto-orchestration explicitly skips grouped tasks, so workflow graphs cannot be driven unattended
- “continuation” has no VK-native incomplete signal; without a signal, automation trends toward either stopping too early or running to a cap

This change introduces **Milestones** as the operator-facing concept for goal-driven work and connects them to existing TaskGroups + auto-orchestration primitives.

## Goals / Non-Goals

**Goals:**
- Add a first-class “goal container” that can hold multiple tasks and a shared objective/definition-of-done.
- Provide milestone-level presets (executor profile and prompt injection) that apply consistently to milestone work.
- Enable bounded, one-step-at-a-time progression:
  - within a task: require an explicit continuation signal (no “budget > 0 means keep going”)
  - across tasks: select only the next eligible milestone node and keep milestone concurrency at 1 active attempt
- Keep human take-over explicit and fast (pause/resume and per-task manual override).
- Provide UI parity for creating, editing, running, and reviewing milestones without inventing a second dashboard.

**Non-Goals:**
- External tracker ingestion (Linear/Jira/GitHub Issues).
- Unbounded “agent keeps running until it figures it out” loops.
- A second executor runtime pipeline or a second review workflow.
- Multi-milestone task membership or cross-project milestones.
- A full “auto planner” that generates an entire task graph from scratch (we make room for it, but v1 keeps planning human-driven).

## Decisions

### 1. Milestones are implemented as an evolution of TaskGroups (not a new parallel container)

TaskGroups already solve the hard parts of multi-task structure:
- stable identity and project scoping
- node/edge graph and “ready vs blocked” semantics
- node instructions, executor profile selection, and attempt starts
- a distinct entry task (`taskKind=group`) for navigation parity with Kanban

Milestone should therefore be a **TaskGroup with additional milestone metadata**, and the UI should use the term “Milestone” for the operator-facing experience.

This avoids duplicating linking tables and keeps milestone editing (add/remove nodes, adjust dependencies) within the existing workflow surface.

### 2. Milestone metadata lives on the TaskGroup record (queryable columns, not only in JSON)

Add explicit DB columns to `task_groups` for milestone concerns:
- `objective` (TEXT, nullable): the desired end-state, written for an agent and a human reviewer.
- `definition_of_done` (TEXT, nullable): acceptance criteria; the runner should stop at review boundaries, not “guess”.
- `default_executor_profile_id` (JSON, nullable): default executor profile for nodes without an override.
- `automation_mode` (enum-ish, default `manual`): whether the scheduler may dispatch milestone nodes unattended.

Rationale:
- columns keep the “what is this milestone aiming for?” view cheap to load
- columns avoid versioning an ad-hoc JSON schema inside `graph_json`
- the graph remains the graph; milestone metadata remains stable across graph edits

### 3. Preset precedence: node override > milestone default > system default

When starting a milestone node attempt (UI or scheduler):
1. If `TaskGroupNode.executorProfileId` is set, use it.
2. Else if `TaskGroup.default_executor_profile_id` is set, use it.
3. Else use the existing system/project default executor profile selection.

This makes milestone presets useful without removing fine-grained per-node tuning.

### 4. One-step-at-a-time progression is enforced by eligibility, not by a background “loop”

Milestone automation must be bounded and predictable:
- At most one in-progress attempt is allowed across all node tasks in a milestone.
- The “next runnable” node is derived from the graph:
  - the node task is not terminal (`done`/`cancelled`)
  - the node has no in-progress attempt
  - all predecessors are `done`
  - checkpoint nodes that require approval are treated as a stop gate until approved

This approach:
- reuses the current scheduler model (poll + claim + start) instead of adding a second orchestrator loop
- avoids races with task finalization by keeping the scheduler as the one writer that starts attempts

### 5. Continuation within a node requires an explicit signal, defaulting to stop

This change does not invent a new completion protocol. It integrates with the continuation signal defined by `add-turn-continuation-orchestration`:
- agents emit a parseable marker (for example `VK_NEXT: continue|review`) in their final message
- missing marker defaults to stop (safe)
- budgets and stop reasons are persisted in shared orchestration state (see that change)

Milestone “definition of done” is injected into the prompt so agents have a stable target when deciding whether to continue.

### 6. UX is data-dense, drill-down, and reuse-first

Use the existing workflow view as the Milestone “detail” surface:
- master (entry) node panel shows milestone objective/DoD + automation toggle + “run next step”
- node panel stays focused on that node: status, blockers, executor, instructions, attempts, follow-up
- milestone progress is summarized with counts (ready/blocked/in-review/done)

This aligns with existing VK patterns (Kanban + panels) and avoids a new parallel “milestone dashboard”.

Accessibility and interaction guardrails (from UI/UX baseline):
- visible focus states for all controls
- no icon-only actions without labels/aria-labels
- avoid layout-shift hover effects; use color/border/shadow transitions

## Risks / Trade-offs

- **Scheduler complexity**: adding graph-aware eligibility increases code paths. → Keep the eligibility computation isolated and unit-tested; add explicit diagnostics when a node is skipped.
- **Prompt bloat**: injecting objective + DoD into every node can grow prompts. → Keep milestone metadata concise and optionally truncatable; prefer “definition of done” as short bullets.
- **Ambiguous “done”**: even with DoD, agents may still stop early. → Safe default is review; continuation requires an explicit marker and budgets.
- **User confusion (TaskGroup vs Milestone)**: renaming must be consistent. → Use “Milestone” in UI, but keep API/internal naming stable until a dedicated rename change is warranted.

## Migration Plan

1. Add DB migrations for new TaskGroup columns with conservative defaults (`automation_mode=manual`).
2. Update Rust models + DTOs and regenerate TypeScript types (`pnpm run generate-types`).
3. Update workflow UI to display/edit milestone fields and rename user-facing labels to “Milestone”.
4. Update scheduler eligibility to allow milestone-managed grouped tasks while keeping non-milestone grouped tasks unscheduled.
5. Add a browser smoke path:
   - create a milestone
   - add 2-3 nodes with a dependency and a checkpoint gate
   - enable milestone automation and observe exactly one node dispatch at a time

## Open Questions

- Do we want milestone `automation_mode` to be independent from project `execution_mode`, or should milestone automation require both to be `auto`?
- Should “run next step” be a scheduler-owned action (enqueue) or a direct “start attempt” API call (more immediate but riskier for concurrency)?
- Do we need a separate milestone MCP tool surface in v1, or is HTTP + existing task MCP sufficient?
