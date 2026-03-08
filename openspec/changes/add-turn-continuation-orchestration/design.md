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

To preserve current behavior, the effective continuation budget should default to zero additional turns unless explicitly enabled.

### 2. Continuation budgets inherit project defaults but tasks may override

Continuation needs a simple override model similar to `automation_mode`:

- Project defines a default continuation budget (recommended default: `0` additional turns).
- Task optionally defines its own continuation budget override:
  - `None` → inherit the project default
  - `0` → explicitly disable continuation for this task even if the project default is non-zero
  - `> 0` → allow up to that many continuation turns for this task, even if the project default is `0`

The scheduler computes an **effective continuation budget** per task from this precedence rule and uses it for eligibility and diagnostics.

### 3. Evaluate continuation only after a successful but incomplete turn

Continuation is considered only when all of the following are true:
- the task is effectively auto-managed
- the latest coding-agent execution completed normally
- the task remains actionable (`todo` / `inprogress` and not terminal)
- the task is not waiting on human review
- there are no pending approvals
- there is no blocking diagnostic
- effective continuation budget and continuation time window still allow another turn

This keeps continuation distinct from retry-on-error recovery.

### 4. Reuse same-session follow-up infrastructure

The scheduler should:
- find the latest coding-agent session for the workspace
- queue or start a `CodingAgentFollowUpRequest`
- continue in the same workspace and session

Continuation remains scheduler-owned rather than UI-owned.

### 5. Use a short, stateful continuation prompt (not a repeated fixed base prompt)

Continuation should not re-send a full static orchestration prompt on every follow-up turn.

Instead, the continuation prompt should be built from the *previous turn's outcome* plus a small continuation instruction template:
- latest turn summary (preferred) and/or a short excerpt of the latest agent "result" message
- current task status and any newly observed blocker/diagnostic context
- remaining continuation budget and any time-window constraints

It should explicitly say:
- this is a continuation turn
- do not restart completed investigation
- focus only on remaining work needed to reach a terminal or review-ready state

This mirrors the intent of Symphony-style momentum: keep the task moving with minimal, stateful reminders instead of restarting from scratch.

### 6. Enforce explicit budgets and stop reasons

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

### 7. Persist only the state needed for auditability

Persist enough continuation state for auditability and diagnostics:
- current continuation turn count
- max continuation turns
- last continuation stop reason
- last continuation prompt timestamp
- effective budget source (`project_default` vs `task_override`) where useful for explainability

These fields should appear only in auto-managed task/task-feed reads. Manual-task surfaces should omit them entirely.

## Migration Plan

- Add continuation policy to the project settings with conservative default-off behavior.
- Add an optional task-level override field with inheritance/disable semantics.
- Persist counters and stop reasons additively so existing tasks remain valid.
- Regenerate shared TypeScript types if continuation diagnostics become part of task DTOs.
- Roll out continuation behind opt-in settings first, then validate with one managed project before broader use.

## Risks / Trade-offs

- Weak budgets could create runaway cost or noisy loops.
- Same-session continuation can accumulate stale assumptions if prompts are not concise.
- Duplicate validations or repeated side effects can occur if eligibility checks are too permissive.
- Even additive diagnostics can clutter the UI if auto-only visibility is not enforced strictly.
