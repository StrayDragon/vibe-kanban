## Why

Vibe Kanban tasks are currently the smallest unit of work, but they do not capture an overall objective that spans multiple tasks. This makes “auto-managed” work brittle: without an explicit, inspectable goal and a continuation signal, automation tends to either stop too early (handoff without progress) or loop until a hard cap (cost is hard to control).

Operators need a higher-level, editable container that defines the acceptance target and policy once, then drives work forward one bounded step at a time with clear handoff points and safe human take-over.

## What Changes

- Introduce a **Milestone** as a first-class project object that groups multiple tasks under one objective and “definition of done”.
- Add milestone-level **presets** for execution (executor profile, reasoning effort, and prompt injection) so every auto-managed step uses a consistent policy.
- Add a bounded **milestone runner** that advances exactly one step at a time:
  - within a task: uses an explicit continuation signal (not “budget > 0”) to decide whether to continue in the same session
  - across tasks: selects the next eligible milestone task/node and starts it when policy allows
- Add UI surfaces to create/edit milestones, review progress, and adjust the plan at any time (including mid-run), with explicit “take over” controls.
- Expose milestone state and next actions via API and (optionally) MCP-friendly read contracts.

## Capabilities

### New Capabilities

- `milestone-orchestration`: Define milestone objectives, presets, and bounded one-step-at-a-time progression across tasks with explicit handoff and take-over.

### Modified Capabilities

- `workflow-orchestration`: Extend task-group/workflow grouping to support milestone-grade objectives and presets, and make milestone progress legible from existing task surfaces.
- `auto-task-orchestration`: Allow scheduler-driven dispatch of grouped work when it is explicitly milestone-managed, while keeping non-milestone grouped tasks unscheduled.
- `task-group-prompting`: Extend prompt augmentation so milestone objective/preset context is injected in addition to node instructions.

## Impact

- **DB**: new milestone persistence (either a new table or an extension of existing task-group persistence), plus new fields for objective, presets, and runner state.
- **Backend**: new/extended routes for milestone CRUD and “run one step”; scheduler integration for milestone-managed dispatch; prompt rendering changes to include milestone context.
- **Frontend**: new milestone creation/edit surfaces and progress views; integrate navigation from tasks to milestones; preserve existing task-level workflows.
- **MCP**: optional read-only “handoff” style payloads for milestone status so agents can decide approve/rework/take-over without log scraping.
