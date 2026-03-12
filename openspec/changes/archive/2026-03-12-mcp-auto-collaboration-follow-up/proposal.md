## Why

VK now has bounded automation primitives (milestone auto-managed work, review handoff, and turn continuation diagnostics), but MCP callers still lack a stable, machine-readable collaboration layer. Today an external agent has to stitch together task detail, attempt summaries, diff summaries, approval state, and activity tails on its own, which makes automation clients fragile, poll-heavy, and too dependent on raw logs.

This change is explicitly scoped to VK's current definition of "auto-managed": milestone node tasks inside milestones with `automation_mode=auto`. Other tasks remain human-managed and should not accidentally inherit automation-only semantics.

## What Changes

- Add a focused read-only MCP handoff contract for review-ready auto-managed tasks.
- Persist explicit control-transfer reasons for human pause / take-over / resume and review handoff transitions.
- Add project-scoped allow-list policy for executor/profile variants requested by MCP-driven auto-managed work.
- Extend existing task/feed reads with orchestration transition diagnostics instead of introducing a separate push channel.
- Mirror the same reasons in existing human task detail and review surfaces only; this change does not create a second MCP-only workflow.

## Capabilities

### New Capabilities
- `mcp-auto-collaboration`: MCP-first review handoff, control-transfer, and policy diagnostics for auto-managed tasks.

### Modified Capabilities
- `auto-task-orchestration`: expose richer control-transfer, handoff, and policy diagnostics in task reads.
- `mcp-task-tools`: add a focused review handoff reader and enrich existing task reads.
- `mcp-activity-feed`: publish orchestration transition events that external orchestrators can consume cheaply.

## Impact

- Backend: `crates/server/src/mcp/task_server/*`, task DTOs, task/activity routes, and orchestration state serialization.
- Config/data model: persisted control-transfer reason plus project executor/profile policy for auto-managed work.
- Frontend: existing task detail and review surfaces show the same reasons and policy diagnostics without adding a parallel inbox.
- External MCP clients: can decide approve / rework / take-over from one handoff read plus existing follow-up tools.

## Reviewer Guide

- This proposal builds on the shipped milestone orchestration model and the existing task orchestration diagnostics state.
- It should NOT widen the definition of "auto-managed" beyond auto milestones: reviewers should check that non-managed tasks do not start emitting misleading orchestration fields.
- The acceptance bar is simple: an MCP caller can understand the state of a review-ready auto-managed task without scraping raw logs.

## Goals

- Let MCP callers approve, rework, or take over *auto-managed* tasks (auto milestone node tasks) through stable, focused read contracts.
- Keep executor escalation explicit and policy-bound at the project level.
- Reuse VK's existing pull-style task/feed model instead of creating a bespoke orchestration transport.

## Non-goals

- Replacing the human review UI with an MCP-only experience.
- Adding external tracker bridges or third-party ticket sync.
- Adding a new long-lived MCP streaming channel.
- Auto-approving, auto-merging, or silently escalating execution rights.

## Risks

- Overloading existing DTOs and making machine consumers harder to stabilize.
- Duplicating summary state between task detail, handoff payloads, and activity feeds.
- Emitting too many transition events and increasing consumer polling complexity.

## Verification

- MCP schema/output tests for the new handoff and transition surfaces.
- Policy tests for allowed and disallowed executor/profile requests.
- Event-feed integration test covering claim, policy rejection, review-ready, and human-takeover transitions.
- One manual smoke check where an MCP client reads a review-ready task and chooses a follow-up action without raw log scraping.
