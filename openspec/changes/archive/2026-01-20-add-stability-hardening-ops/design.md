## Context
- Entry-indexed log streams currently error on broadcast lag, interrupting live clients and DB persistence.
- Agent availability checks exist, but are not surfaced during attempt creation; GitHub CLI preflight is missing.
- Ops guidance for SQLite locks/backups and routine checks is not documented.

## Goals / Non-Goals
- Goals: resync entry-indexed log streams after lag without closing; provide preflight status for agent and GitHub CLI; document SQLite lock/backup practices and ops checklists.
- Non-Goals: change log storage formats, add new setup helpers, or redesign PR workflows.

## Decisions
- Implement resync in `MsgStore` entry streams by detecting lagged broadcast receives and emitting a snapshot of current entries (append/replace) before continuing live events.
- Add a GitHub CLI availability endpoint that reports `installed/authenticated` vs `not_installed` / `not_authenticated`, and reuse existing agent availability checks.
- Surface preflight status in attempt creation and PR creation dialogs to warn or gate actions before execution.
- Publish ops guidance in a new `docs/operations.md` (linked from `README.md`).

## Risks / Trade-offs
- Resync may re-send entries; index-based upserts mitigate duplicate effects for log entry streams.
- Preflight gating may block users who prefer to proceed; UI should allow explicit overrides or provide clear remediation.

## Migration Plan
- No data migrations. Add new endpoints/types and UI wiring alongside docs.

## Open Questions
- Should preflight block attempt creation or only warn?
- Should PR creation preflight run automatically on dialog open or only on action?
