## 1. Archived Kanban Derivation Slimdown

- [x] 1.1 Refactor `frontend/src/hooks/archived-kanbans/useArchivedKanbanTasks.ts` to use `taskDerivation` incremental cache + WS `invalidate.taskIds` (remove `Object.values + sort + per-status sort` full rebuild on every patch).
- [x] 1.2 Ensure stable ordering (`created_at` desc, tie by `id`) and preserve referential stability for unaffected status lists when applying localized updates.

## 2. Tests

- [x] 2.1 Add unit tests for `frontend/src/hooks/tasks/taskDerivation.ts` covering insert/delete/same-status update/status move ordering and reference stability expectations.
- [x] 2.2 Add regression coverage for archived kanban derivation (ensuring it reuses `taskDerivation` and maintains sorting + per-status stability).

## 3. Verification

- [x] 3.1 Run `pnpm -C frontend run check`, `pnpm -C frontend run lint`, and `pnpm -C frontend run test`.
- [x] 3.2 Run `just qa` and `just openspec-check`.
