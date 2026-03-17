## Context

Sessions are stored in the `sessions` table and exposed via `/api/sessions`.
Sessions are created in multiple backend flows (initial agent run, setup flows,
PR flows, etc.) but the current schema only includes an `executor` string and
timestamps.

The frontend does not currently provide a dedicated "session list" UI; it
primarily shows execution processes and normalized logs. The Processes dialog is
the most direct place where a user can navigate between many runs and would
benefit from session names.

## Goals / Non-Goals

**Goals:**
- Add an optional `Session.name` field that can be:
  - auto-generated on creation when not provided
  - renamed by the user
- Provide a stable API for renaming a session.
- Surface session names in the Processes dialog and allow renaming.

**Non-Goals:**
- Do not redesign the entire attempt/task UI around sessions.
- Do not add full-text search over session names in this change.

## Decisions

### Decision: Add nullable `name` column to `sessions`

We will add `name TEXT NULL` to the `sessions` table via a SeaORM migration.
`name` remains optional to preserve compatibility.

Validation rules (server-side):
- Trim leading/trailing whitespace.
- Empty string SHALL be treated as `NULL`.
- Enforce a max length (default: 120 chars) to avoid UI breakage.

### Decision: Rename via `PATCH /api/sessions/:session_id`

Add an endpoint:
- `PATCH /api/sessions/:session_id`
- Body: `{ "name": string | null }`
- Response: updated `Session`

This endpoint only mutates the session name; it does not touch executor or other
state.

### Decision: Auto-naming is best-effort and flow-specific

Auto-naming applies only when no explicit name is provided.

Initial naming rules (defaults):
- Coding agent initial session: `Run: <task.title>` (truncate to 80)
- Setup helper sessions (Codex/Cursor/GH CLI setup): `<Tool> Setup`
- Fallback: `Session <short-id>` (derived from UUID prefix)

We will implement naming at session creation call sites where the necessary
context is available (task title, flow type). If a call site cannot provide
context, it can omit a name and rely on the fallback.

### Decision: First UI surface is the Processes dialog

We will extend the Processes dialog to:
- Fetch sessions for the current workspace/attempt
- Display a session selector showing `name` (or fallback label)
- Provide a rename action for the selected session
- (Optional) filter execution processes by selected session id

This makes the feature immediately usable without requiring a larger navigation
redesign.

## Risks / Trade-offs

- **[Many call sites create sessions]** → start with best-effort auto-naming at
  the key call sites; do not block on perfect coverage.
- **[UI complexity]** → keep the UI small (selector + rename) and avoid heavy
  refactors.

## Migration Plan

1. Add DB migration + SeaORM entity/model updates.
2. Update `/api/sessions` responses to include `name`.
3. Add rename endpoint.
4. Regenerate TS types.
5. Update Processes dialog to display/rename session names.

## Open Questions

- Should names be localized? (Default: **no**, keep stored values in English.)
- Should clearing the name re-run auto-naming, or just show fallback? (Default:
  show fallback label; do not rewrite historical names.)

