## Context

This change adapts Symphony's workspace hooks to VK's own lifecycle boundaries instead of copying the tracker-centric design directly.

The VK-equivalent phases are:
- `after_prepare`: after a workspace/worktree is materialized, repo files are ready, and VK has written any generated workspace files
- `before_cleanup`: immediately before explicit worktree removal or background workspace cleanup

These hooks remain optional and project-scoped.

## Goals / Non-Goals

**Goals:**
- Let each project declare baseline workspace bootstrap and cleanup behavior.
- Keep hook execution workspace-bound, auditable, and failure-policy aware.
- Surface hook outcomes in existing task/workspace diagnostics so humans and automation can explain startup or cleanup failures.

**Non-Goals:**
- Replace `run-agent-setup` or any executor-specific setup flow.
- Add per-task hook customization in this first version.
- Build a generic background job runner or CI pipeline abstraction.
- Add manual rerun controls before lifecycle semantics are proven.

## Decisions

### 1. Use two explicit project-scoped phases

Project configuration adds two independent phases:
- `after_prepare`
- `before_cleanup`

Recommended per-phase shape:
- `command`
- `working_dir` (workspace-relative only)
- `failure_policy`
- `run_mode`

Suggested policies:
- `after_prepare.failure_policy = block_start | warn_only`
- `before_cleanup.failure_policy = warn_only | block_cleanup`
- `after_prepare.run_mode = once_per_workspace | every_prepare`

Default remains disabled.

### 2. Run `after_prepare` only after workspace materialization is complete

`after_prepare` executes after `ensure_container_exists` completes and after VK writes any generated workspace config files.

Typical use cases:
- dependency bootstrap
- repo-local trust/setup
- repo-local environment preparation
- generated file hydration required before the first agent turn

When `failure_policy = block_start`, both manual starts and auto-managed dispatch must stop before coding-agent execution begins and expose the failure as a structured diagnostic.

### 3. Run `before_cleanup` at actual deletion boundaries

`before_cleanup` executes before explicit remove-worktree actions and before background cleanup flows delete workspace directories.

Typical use cases:
- collecting repo-local artifacts
- invoking cleanup scripts
- removing generated state that should not linger in worktrees

Default behavior is best-effort; removal is only blocked when the project explicitly opts into `block_cleanup`.

### 4. Keep hook execution inside existing guardrails

Hook commands should reuse VK's current execution restrictions as much as possible:
- workspace-relative working directory only
- explicit command storage in project config
- auditable logs/outcomes
- no hidden global shell execution outside the workspace root

This preserves VK's local-first trust model.

### 5. Persist latest hook outcome separately from setup-complete signals

Do not overload `workspace.setup_completed_at` to mean “all hooks succeeded”. Persist dedicated hook outcome state, such as:
- latest hook phase run
- latest hook outcome
- latest hook error summary
- phase timestamps where useful

A lightweight hook run record or execution-log linkage is acceptable if it improves debugging, but the first version only needs enough persisted state to explain the latest outcome.

### 6. Wire lifecycle ownership to existing runtime boundaries

- workspace preparation code owns `after_prepare` timing
- start/dispatch orchestration owns block-start semantics
- explicit remove-worktree and background cleanup flows own `before_cleanup`

This keeps hook logic close to the boundaries it affects instead of creating a parallel scheduler.

### 7. Keep human surfaces minimal and existing

This change should only add:
- project settings controls for hook configuration
- latest hook result in existing workspace/task/attempt detail surfaces
- structured non-dispatch diagnostics for auto-managed tasks when blocking hooks fail

A dedicated hook dashboard or rerun button is follow-up work.

### 8. Keep `run-agent-setup` as a separate concern

`run-agent-setup` remains useful for executor-specific helpers such as Codex or Cursor. Workspace lifecycle hooks are broader and project-owned:
- hooks prepare the workspace regardless of executor
- setup helpers remain executor-aware and on-demand

The two should coexist rather than replace each other.

## Migration Plan

- If project settings are versioned config, add a new latest config version with disabled hooks as the default.
- Add persisted latest-outcome fields additively so existing workspaces remain valid.
- Regenerate shared TypeScript types if hook settings or diagnostics are exposed in DTOs.
- Do not backfill historical hook runs; start recording outcome from first execution after rollout.

## Risks / Trade-offs

- Hook duplication is possible if repeated prepare calls are not paired with clear run-mode semantics.
- Synchronous hook execution can increase startup latency for large mono-repos.
- Operators may confuse hook failures with executor/runtime failures unless diagnostics remain distinct.
