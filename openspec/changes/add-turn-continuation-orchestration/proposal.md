## Why

Symphony's continuation loop keeps the same task moving across multiple agent turns instead of treating the first normal completion as the end of unattended work. VK already has same-session follow-up primitives, but auto-managed tasks currently stop after a single unattended turn. That leaves multi-step internal tasks under-automated and pushes partial progress back to humans earlier than necessary.

## What Changes

- Add optional, bounded same-session turn continuation for auto-managed tasks only.
- Keep the feature disabled by default so current manual and one-turn auto behavior remain unchanged until a project opts in.
- Reuse existing follow-up/session infrastructure instead of creating a second attempt or a new workspace per continuation turn.
- Add strict continuation stop conditions around review handoff, approvals, blocking diagnostics, and continuation budgets.
- Expose continuation state and budget diagnostics only in auto-managed task surfaces and MCP-readable task data.

## Capabilities

### New Capabilities
- `turn-continuation-orchestration`: bounded same-session continuation for auto-managed tasks after a normal turn completion.

### Modified Capabilities
- `auto-task-orchestration`: expose continuation counters, stop reasons, and eligibility diagnostics for auto-managed work.

## Impact

- Backend: scheduler/orchestrator loop, queued follow-up handling, task dispatch state, and attempt/session diagnostics.
- Config/data model: project-level continuation policy plus persisted counters and stop reasons for managed tasks.
- UI/MCP: only auto-managed task detail and machine-facing reads explain continuation progress or stop reasons.
- Cost/safety: continuation budgets become a first-class runtime concern.

## Reviewer Guide

- This proposal only applies when optional auto orchestration is enabled; manual projects and manual tasks must remain behaviorally unchanged.
- It can be implemented independently of workspace hooks and MCP collaboration, though those changes improve the overall unattended workflow.
- The acceptance bar is narrow: an eligible managed task can continue in the same session for another bounded turn, then stop for a clear reason.

## Goals

- Improve completion rate for multi-step auto-managed tasks without creating extra workspaces.
- Reuse VK's existing session/follow-up model instead of cloning Symphony's tracker loop.
- Keep continuation tightly bounded, human-review-aware, and invisible to manual-task workflows.

## Non-goals

- Copying Symphony's external tracker state loop directly.
- Running unlimited autonomous turns.
- Replacing retry-on-error with continuation-on-success; both remain distinct.
- Adding continuation indicators, settings, or behavioral changes to tasks that remain manual.

## Risks

- Runaway loops and unexpected cost growth if continuation budgets are too weak.
- Repeated validation or duplicate side effects across continuation turns.
- Confusing humans if continuation stop reasons are not surfaced clearly in managed-task diagnostics.
- Session quality may degrade if prompts do not stay concise and stateful.

## Verification

- Scheduler tests for continuation eligibility, budget exhaustion, and stop reasons.
- Session/follow-up integration tests proving same-session continuity.
- Manual smoke check showing a task continues automatically for one additional turn and then stops at a review handoff boundary.
