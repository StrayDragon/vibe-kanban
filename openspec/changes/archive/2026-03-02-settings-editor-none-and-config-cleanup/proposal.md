## Why

Some users do not want Vibe Kanban to open files/projects in an external editor (or they run without a local editor binary on `PATH`). Today, editor-related affordances (navbar icon, “Open in …” actions, availability warnings) can add noise even when the feature is undesired.

Separately, skipping git hooks via `--no-verify` is often project-dependent. A single global toggle is too coarse: users may want it enabled by default but disabled for specific projects (or vice versa).

Finally, the Settings surface area is growing. The “Pull Requests” behavior and “Remote SSH Host” editor setting may be niche and worth revisiting to reduce complexity.

## Goals

- Add an explicit “None / Do not use” editor type that disables editor integration and hides all “Open in …” affordances.
- Allow `--no-verify` behavior to be configured globally and overridden per project, with project settings taking precedence.
- Clearly explain the precedence rules in the UI.
- Audit whether “Pull Requests” settings and “Remote SSH Host” should remain in Settings.

## Non-goals

- Redesign the entire Settings UI or navigation.
- Change the default editor selection for existing users.
- Change git behavior beyond adding a project override for `--no-verify`.

## What Changes

- Add a new `EditorType` value representing “None / disabled”.
- When editor type is disabled:
  - Hide all “Open in …” UI prompts/actions (including the navbar IDE icon).
  - Do not show editor availability checks/warnings.
  - Backend “open editor” endpoints return a clear, non-crashing response (e.g., a validation error) if called.
- Add a project-scoped override for git `--no-verify`:
  - Global setting remains as the default.
  - Project setting (when explicitly set) overrides the global value.
  - UI copy explains this behavior (e.g., “Project settings override global”).
- Research outcomes (no user-visible change until decided):
  - Determine whether “Pull Requests” settings and “Remote SSH Host” should be kept, moved under an “Advanced” area, or removed (with migration plan if removed).

## Capabilities

### New Capabilities
- `editor-integration`: Users can disable editor integration (“None”) and the app hides all “Open in …” affordances.
- `project-git-hooks`: Projects can override the global git hook skipping behavior (`--no-verify`).

### Modified Capabilities
- (none)

## Impact

- Backend: `EditorType` gains a new variant; “open editor” endpoints must handle disabled state.
- Frontend: multiple components must gate rendering of “Open in …” affordances; Settings copy updated.
- DB: add a nullable per-project field for git hook behavior override.
- Shared types: requires regenerating TypeScript types after Rust changes (`pnpm run generate-types`).

## Risks

- Missing a UI affordance could leave a stray “Open in …” entry visible even when disabled.
- Adding a per-project override touches DB/API/types and needs careful backwards compatibility (null = inherit).

## Verification

- Frontend smoke checks:
  - Selecting “None” hides the navbar IDE button and any “Open in …” actions for attempts/projects.
  - Editor availability warnings do not appear when disabled.
- Backend checks:
  - “Open editor” endpoints return a clear error when editor integration is disabled.
  - Git operations that use `--no-verify` respect project override precedence (project overrides global).
