## Why

Symphony's `after_create` and `before_remove` hooks map closely to VK's workspace lifecycle, but VK currently relies on ad hoc setup helpers and manual cleanup around workspace creation/removal. That leaves repetitive bootstrap work for mono-repos, repo-local trust/setup, and generated artifact cleanup outside the platform's normal task lifecycle.

## What Changes

- Add optional project-scoped workspace lifecycle hooks for the VK-equivalent phases `after_prepare` and `before_cleanup`.
- Reuse existing workspace/path guardrails so hook commands stay workspace-bound and auditable.
- Record hook execution outcome so both manual and auto-managed flows can explain whether bootstrap or cleanup actually ran.
- Keep agent-specific setup helpers such as `run-agent-setup` separate; lifecycle hooks define project baseline setup, not executor-specific behavior.

## Capabilities

### New Capabilities
- `workspace-lifecycle-hooks`: project-scoped, guarded lifecycle hooks for workspace preparation and cleanup.

### Modified Capabilities
- `workspace-management`: workspace removal and cleanup flows honor configured lifecycle hooks.
- `auto-task-orchestration`: auto-managed dispatch waits for blocking preparation hooks and exposes failures as diagnostics.

## Impact

- Backend: workspace creation/removal paths in `crates/tasks`, `crates/server`, and `crates/execution`.
- Config/data model: project hook configuration plus persisted latest hook outcome.
- Frontend: project settings and existing workspace/task detail surfaces show hook configuration and latest result.
- Operations: teams can standardize workspace bootstrap/cleanup without requiring executor-specific scripting.

## Reviewer Guide

- This proposal is independent of MCP collaboration and continuation work, but it strengthens the runtime foundation those changes may rely on later.
- The core acceptance bar is lifecycle correctness: a project can opt into guarded hooks and VK executes them at the right boundary with the right failure policy.
- Manual rerun tooling is intentionally out of scope for this proposal; the first version focuses on lifecycle execution and visibility.

## Goals

- Eliminate repeated manual bootstrap work after workspace creation.
- Provide predictable, auditable cleanup before workspace removal.
- Fit naturally into both manual and auto-managed attempt flows.

## Non-goals

- Running arbitrary global shell automation outside the workspace.
- Replacing dev scripts or executor-specific setup helpers.
- Creating per-task hook definitions or a general CI/CD hook system.
- Adding a standalone hook-run console or manual rerun workflow in this change.

## Risks

- Flaky hooks can block task start or cleanup if failure policy is unclear.
- Re-running hooks on every prepare could be expensive or destructive.
- Long-running hooks can hurt perceived attempt startup latency.

## Verification

- Workspace creation/removal tests covering hook success, failure, run-mode behavior, and idempotence.
- Guardrail tests for working directory validation and forbidden command shapes.
- Manual smoke check covering one project with a bootstrap hook and one with a cleanup hook.
