## Context

This change is worth pursuing, but only as a bounded VK-native continuation model.

Symphony's value is not “more turns” by itself. Its real value is maintaining momentum while the same task is still active. VK already has the necessary low-level pieces:
- same-session follow-up requests
- queued follow-up execution after a process completes
- session summaries and latest agent session lookup

That means VK can implement continuation without introducing a tracker-driven loop, extra workspaces, or any change to manual-task behavior.

## Goals / Non-Goals

**Goals:**
- Let eligible auto-managed tasks continue in the same session after a normal incomplete turn.
- Keep continuation bounded by explicit turn/time budgets and human-review gates.
- Reuse existing follow-up/session infrastructure instead of adding a second execution pipeline.
- Make continuation outcomes inspectable to humans and MCP clients when automation is enabled.

**Non-Goals:**
- Change manual project or manual task behavior in any way.
- Add a new workspace, new attempt type, or tracker-style orchestration loop.
- Hide uncertainty behind unlimited “one more turn” retries.
- Create standalone continuation UI outside existing managed-task diagnostics.

## Decisions

### 1. Continuation is auto-only and default-off

Continuation should only exist within optional auto orchestration. Manual projects and manual task overrides must not:
- schedule continuation turns
- persist user-visible continuation counters as a manual-task concern
- render continuation affordances in human-manual task views

To preserve current behavior, the project-level continuation budget should default to zero additional turns until explicitly enabled.

### 2. Evaluate continuation only after a successful but incomplete turn

Continuation is considered only when all of the following are true:
- the task is effectively auto-managed
- the latest coding-agent execution completed normally
- the task remains actionable (`todo` / `inprogress` and not terminal)
- the task is not waiting on human review
- there are no pending approvals
- there is no blocking diagnostic
- continuation budget and continuation time window still allow another turn

This keeps continuation distinct from retry-on-error recovery.

### 3. Reuse same-session follow-up infrastructure

The scheduler should:
- find the latest coding-agent session for the workspace
- queue or start a `CodingAgentFollowUpRequest`
- continue in the same workspace and session

Continuation remains scheduler-owned rather than UI-owned.

### 4. Use a short continuation prompt

The continuation prompt should be built from:
- latest turn summary
- current task status
- remaining continuation budget
- any newly observed blocker/diagnostic context

It should explicitly say:
- this is a continuation turn
- do not restart completed investigation
- focus only on remaining work needed to reach a terminal or review-ready state

### 5. Enforce explicit budgets and stop reasons

Recommended policy fields:
- max continuation turns per attempt
- max elapsed continuation window
- optional cool-down between turns

Continuation must stop immediately when:
- task enters review or done/cancelled
- an approval is pending
- a blocking diagnostic appears
- a policy violation occurs
- budget is exhausted

Persist a structured stop reason so humans and MCP clients can tell why automation stopped.

### 6. Persist only the state needed for auditability

Persist enough continuation state for auditability and diagnostics:
- current continuation turn count
- max continuation turns
- last continuation stop reason
- last continuation prompt timestamp

These fields should appear only in auto-managed task/task-feed reads. Manual-task surfaces should omit them entirely.

## Migration Plan

- Add continuation policy to the latest project automation config version with conservative default-off behavior.
- Persist counters and stop reasons additively so existing tasks remain valid.
- Regenerate shared TypeScript types if continuation diagnostics become part of task DTOs.
- Roll out continuation behind opt-in settings first, then validate with one managed project before broader use.

## Risks / Trade-offs

- Weak budgets could create runaway cost or noisy loops.
- Same-session continuation can accumulate stale assumptions if prompts are not concise.
- Duplicate validations or repeated side effects can occur if eligibility checks are too permissive.
- Even additive diagnostics can clutter the UI if auto-only visibility is not enforced strictly.
