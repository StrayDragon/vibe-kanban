## Why

Users often want a high-quality plan before allowing an agent to execute code
changes. A dedicated "plan-only" mode makes this workflow safer and faster:

- The agent produces an actionable plan (steps/todos) without running commands
  or modifying files.
- The UI can surface the plan for review and then allow the user to start a
  normal execution run if desired.

Upstream Vibe Kanban introduced Codex plan mode; we should provide the same
capability in our Codex executor integration.

## What Changes

- Add a `plan` flag to the Codex executor config (`crates/executor-codex`) to run
  Codex in plan-only mode.
- Add a CODEX `PLAN` profile variant in `crates/executors/default_profiles.json`
  so users can select it in the existing profile selector UI.
- Enforce plan-only constraints:
  - no command execution
  - no filesystem writes / patch application
  - read-only sandbox defaults
- Ensure plan output is surfaced via existing Todo/Plan UI (Codex `PlanUpdate`
  events are already normalized into TodoManagement entries).

## Capabilities

### New Capabilities

- `codex-plan-mode`: Codex can run in a "plan-only" mode that produces a plan
  and exits without mutating the workspace.

### Modified Capabilities

<!-- None -->

## Impact

- `crates/executor-codex`: config schema + client handling for plan mode.
- `crates/executors/default_profiles.json`: add CODEX `PLAN` variant.
- UI: minor labeling/affordances for plan-only runs (optional; scope-limited).

## Goals / Non-Goals

**Goals:**
- Provide a safe plan-only run mode for Codex.
- Reuse the existing Todo panel to display structured plans.

**Non-Goals:**
- No full "model discovery" UI like upstream monorepo.
- No changes to non-Codex executors in this change.

## Risks

- Codex might still attempt tool calls → must enforce server-side rejection in
  plan mode.
- Ambiguity around what counts as "mutation" → default to strict: deny
  `apply_patch` and command execution.

## Verification

- Start an attempt using CODEX `PLAN` and confirm:
  - no files are modified
  - no commands run
  - a plan appears in the Todo panel
- `pnpm run backend:check` + `pnpm run check` + `cargo test --workspace`.

