## Context

We integrate Codex via `crates/executor-codex`, spawning the Codex app-server and
normalizing its streamed events into our log store. The frontend already renders
Todo items derived from Codex `PlanUpdate` events (normalized as a `plan` tool
entry with `TodoManagement` action type).

What is missing is a first-class "plan-only" execution mode that guarantees the
agent cannot mutate the workspace.

## Goals / Non-Goals

**Goals:**
- Add a Codex plan-only mode selectable via executor profile variant.
- Enforce: no writes, no command execution.
- Preserve existing normal execution behavior for Codex when plan mode is off.

**Non-Goals:**
- Do not redesign our executor selection UI.
- Do not implement upstream's full model-selector discovery stack.

## Decisions

### Decision: Add `plan: bool` to Codex executor config

We will add a `plan` boolean to the Codex executor config struct so it can be
enabled via profiles (e.g., CODEX `PLAN`).

### Decision: Enforce plan-only constraints in the app-server client

Plan-only mode must be enforced by the host, not just by "instructions", to
avoid accidental mutation.

In plan mode:
- Reject tool calls that can mutate state (e.g., apply_patch, run_command,
  write_file-like tools).
- Default to a read-only sandbox configuration for extra defense-in-depth.

### Decision: Surface plans via existing Todo panel

We already normalize Codex `PlanUpdate` into TodoManagement entries. We will not
add a new plan UI; instead, we ensure plan mode reliably emits `PlanUpdate`
events and we may add a small "Plan-only" label in relevant UI surfaces.

## Risks / Trade-offs

- **[Tool classification]** → define a strict allowlist for plan mode (read-only
  tools only). Anything unknown is rejected.
- **[User expectations]** → clearly label plan mode in the profile selector
  (variant name `PLAN`) and optionally in the attempt UI.

## Migration Plan

1. Add `plan` field to Codex config and plumb it through spawn/client layers.
2. Add CODEX `PLAN` profile variant in `crates/executors/default_profiles.json`
   with read-only sandbox defaults.
3. Add/adjust minimal UI labeling if needed.
4. Add regression tests for "plan mode denies mutation tools".

## Open Questions

- Should plan mode still create a workspace/worktree? (Default: **yes**; it may
  need to read repo state. The sandbox remains read-only.)
- Should we allow read-only command execution (e.g., `rg`, `cat`) in plan mode?
  (Default: **no** initially; keep strict and expand later if needed.)

