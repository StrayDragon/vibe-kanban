## Why

Symphony's strongest workflow advantage is that operators do not need to babysit every task start, retry, and recovery step. `vk` historically required manual task selection and manual restart loops, which makes internal task pools expensive to operate when many small tasks accumulate.

A first phase of optional project-level auto orchestration has already been implemented in code. This change backfills the missing OpenSpec artifacts for that shipped foundation, documents its current limits, and defines the next increment: better user/MCP control surfaces, task-level automation override, and clear "why not scheduled" diagnostics.

## What Changes

- Backfill an OpenSpec change for the already-landed phase-1 optional auto orchestration foundation for internal `vk` tasks.
- Preserve manual mode as the default and keep human-driven task execution fully supported.
- Add and document project-level execution mode and scheduler controls for concurrency and retries.
- Persist scheduler dispatch state per task and expose automation state in task responses and UI badges.
- Reuse the existing `task -> workspace -> session -> execution_process` runtime path instead of introducing a second executor pipeline.
- Extend the design scope with a better switch-based UX so both interactive users and programmatic callers can opt projects/tasks into auto-managed operation.
- Define a repo-versioned auto-orchestration prompt contract adapted from `../symphony` so unattended runs use `vk`-native task/retry/review instructions instead of tracker-specific wording.
- Add follow-up scope for per-task automation override and operator-visible diagnostics explaining why a task was not scheduled.

## Goals

- Reduce operator babysitting for internal task pools without removing manual control.
- Keep auto orchestration optional, incremental, and compatible with the current runtime model.
- Make automation state visible enough that users can understand whether a task is running, retrying, blocked, or waiting for review.
- Provide a clean path to task-level override and MCP-friendly control surfaces instead of forcing everything through project-wide settings.
- Keep the unattended agent prompt policy versioned in-repo so automation behavior can evolve alongside code review and orchestration rules.

## Non-goals

- Pulling work from external trackers such as Linear, Jira, or GitHub Issues in this change.
- Auto-scheduling task-group or grouped-child tasks before dependency-aware orchestration exists.
- Replacing the existing attempt/runtime pipeline.
- Defining proof bundles or richer execution quality gates in this change.

## Risks

- Automation can surprise operators if control surfaces and state labels are unclear.
- Scheduler state can drift from runtime state if retries, review handoff, or blocked conditions are not reconciled consistently.
- Project-level auto mode alone may be too coarse until task-level override is added.

## Verification

- `cargo check -p db -p repos -p tasks -p app-runtime -p server`
- `cargo test -p server auto_orchestrator::tests::retry_backoff_is_capped`
- `cargo test -p repos validate_dev_script_update -- --nocapture`
- `pnpm run check`
- `pnpm run frontend:lint`
- Manual smoke check: switch a project between manual and auto, confirm badges/settings update, and confirm eligible internal tasks can be auto-started while grouped tasks remain unscheduled.

## Capabilities

### New Capabilities
- `auto-task-orchestration`: optional automatic dispatch, retry, review handoff, operator control, and scheduling diagnostics for internal `vk` tasks.

### Modified Capabilities
- None.

## Impact

- **Code:** `crates/db`, `crates/repos`, `crates/tasks`, `crates/server`, `frontend/src/pages/settings/ProjectSettings`, task/project UI components, and generated shared types.
- **Data:** new project automation fields plus persisted per-task dispatch state.
- **APIs:** task payloads now include orchestration metadata; follow-up work will extend task/project mutation surfaces for finer-grained control.
- **Runtime:** a background scheduler loop now reconciles eligible projects/tasks and starts attempts through the existing orchestration path.
- **Prompts:** follow-up work will introduce a versioned `vk` auto-orchestration workflow prompt adapted from `../symphony/elixir/WORKFLOW.md`.
- **Docs:** `docs/auto-orchestration.md` captures the shipped concept comparison and runtime overview.

## Extension: Human-first orchestration UX and agent-managed follow-up scope

The shipped foundation now already includes task-level automation override, visible diagnostics, and the repo-versioned unattended prompt. The next increment should stop treating automation as a hidden scheduler detail and instead present it as an explicit, human-readable operating mode inside the task UI.

### Added follow-up scope

- Make manual vs auto-managed ownership visually obvious in list, detail, and project surfaces without relying on color alone.
- Add a human review inbox / handoff surface so operators can quickly consume results from auto-managed runs the way Symphony uses a persistent workpad + `Human Review` state.
- Reuse the current task/attempt/session data model as far as possible for review summaries, validations, and next actions.
- Define a safe way for MCP/agent callers to create related follow-up tasks that can remain manual by default or opt into automation under project policy.
- Keep project-wide automation enablement higher-friction than task-level auto requests so external agents cannot silently convert an entire project into managed mode.

### Additional goals

- Preserve `vk` as a human-first UI even when auto orchestration is enabled.
- Make it obvious who currently owns execution of a task: a human, the scheduler, or a human-review handoff state.
- Give humans a fast review path for auto-managed results using existing summaries/diffs/approvals before adding heavier workflow state.
- Let other agents call MCP to request auto-managed execution for specific tasks, while keeping project-wide policy explicit and reviewable.

### Additional risks

- If ownership is shown only as a small badge, operators may still miss whether a task is safe to manually take over.
- If agent-created follow-up tasks are not attributed, humans will not know why a new task exists or whether it is safe to auto-run.
- If MCP can escalate project-wide automation too easily, automation policy becomes surprising and hard to audit.

### Additional impact

- **UI:** task lists/details need an ownership lane, review lane, and clearer action hierarchy.
- **Data:** current state is sufficient for the first handoff UX, but related-task lineage likely needs one more relation/source field.
- **MCP:** task-level auto requests are already viable; project-level automation should be an explicit future capability with policy guardrails.

## Review against `../symphony`

This review confirms that the current `vk` change has reached conceptual parity with the strongest **internal orchestration** parts of Symphony, while intentionally stopping short of Symphony's external tracker shell.

### Confirmed parity for `vk`-native tasks

- polling scheduler that claims eligible work and reuses the existing attempt/runtime pipeline
- bounded retry/backoff with persisted dispatch state
- unattended prompt posture with retry/continuation wording
- explicit human-review handoff instead of silent redispatch after a successful run
- lineage/attribution for agent-created follow-up tasks
- human-facing visibility for managed vs manual vs needs-review task states

### Confirmed gaps versus Symphony

These remain intentionally out of scope for the current shipped implementation and should be treated as the next increment rather than retroactively assumed complete:

- external tracker ingestion and tracker state mutation (Linear/Jira/GitHub issues)
- dedicated orchestration operations dashboard with retry pressure / refresh timing / token pressure views
- stall detection, stale-claim recovery, and more opinionated workspace cleanup semantics
- tracker-native review / merge choreography

## Next increment: MCP / agent-friendly orchestration

To make optional auto orchestration work well when the caller is another agent, MCP client, or external controller, the next increment should focus on **coordination**, not just dispatch.

### Desired outcome

A human and an agent should be able to collaborate on the same project without hidden control transfer, silent escalation, or ambiguity about why a task is or is not being scheduled.

### Scope to add next

- explicit human ↔ agent control-transfer semantics on task handoff surfaces
- MCP-readable handoff summary surface so a non-human caller can consume review outcomes without scraping raw logs
- safe policy around agent-selected executor/profile variants for auto-managed work
- stronger scheduler diagnostics for stale claims / reclaim / cleanup conditions
- optional event-oriented surfaces for automation transitions so MCP callers do not need inefficient polling loops

