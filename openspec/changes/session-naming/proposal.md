## Why

As users run more attempts and follow-up actions, a single workspace can contain
multiple sessions. Today sessions are only identifiable by opaque IDs and (at
best) an executor string. This makes it hard to:

- Quickly find "the session where X happened"
- Distinguish setup/utility sessions from coding sessions
- Reuse or refer to specific sessions in UI and tooling

Upstream Vibe Kanban added session naming (rename + auto-naming) to improve
navigation and context.

## What Changes

- Add an optional `name` field to `Session` (DB + API + generated TS types).
- Add a session rename endpoint so the UI can update a session name.
- Implement simple auto-naming for sessions created by common flows (coding
  agent run, setup flows), without overriding user-provided names.
- Surface session names in the UI (initially in the Processes dialog) and allow
  renaming from the UI.

## Capabilities

### New Capabilities

- `session-naming`: Sessions have a human-friendly name that can be auto-set and
  user-renamed.

### Modified Capabilities

<!-- None -->

## Impact

- DB schema for `sessions` (SeaORM migration + entity/model updates).
- Backend routes under `/api/sessions/*`.
- Frontend Processes dialog (`frontend/src/components/tasks/TaskDetails/*`) to
  display and rename sessions.
- Regenerate `shared/types.ts` via `pnpm run generate-types` after Rust type
  changes.

## Goals / Non-Goals

**Goals:**
- Make sessions easy to distinguish and reference in UI.
- Ensure naming is stable and does not break existing consumers (name is
  optional).

**Non-Goals:**
- No global search/indexing over session names (can be added later).
- No changes to execution-process semantics or log formats.

## Risks

- DB migration introduces new column → keep it nullable and backward compatible.
- UI placement ambiguity → pick a concrete first surface (Processes dialog) and
  keep the rest optional.

## Verification

- Create a new attempt and confirm the latest session has an auto-generated
  name.
- Rename a session in the UI and refresh; the name persists.
- `pnpm run generate-types` + `pnpm run check` + `cargo test --workspace`.

