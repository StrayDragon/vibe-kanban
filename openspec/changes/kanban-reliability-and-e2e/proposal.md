## Why

Task/project management is only trustworthy if the UI reflects mutations immediately and safely. Today, multiple high-frequency actions (task create/edit/status change/drag/delete) can succeed at the API layer while the Kanban UI remains stale until the user navigates away and back. This breaks user confidence, causes “ghost cards”, and makes it easy to take the wrong follow-up action.

In addition, a column-level “Add task” affordance is present but effectively non-interactive, and project creation/deletion flows have mis-click risk (a repo list click can immediately create a project; deletion confirmation does not disambiguate same-name projects).

## What Changes

- Make task mutations (create/update/delete/status drag) update the Kanban UI immediately, then reconcile with realtime streams:
  - Use optimistic updates and/or cache updates on mutation success.
  - Add a reliable resync path when the stream misses an update (avoid “switch away and back” as the only recovery).
  - Handle async/202-style deletes as “remove now, confirm later / rollback on failure”.
- Fix the per-column “Add task” control to be clickable, focusable, and testable (add stable selectors) and to open the same create-task flow as the global create button.
- Make project creation and deletion safer:
  - Replace implicit “click repo → project is created” with an explicit wizard: choose repo → name project → confirm create.
  - Disambiguate same-name projects in UI/confirmations (show repo path/ID).
  - Prevent or strongly warn on selecting temporary worktree directories as a repo source.
- Reduce i18n + a11y regressions on the touched surfaces:
  - Remove/replace hard-coded strings on the affected dialogs/pages.
  - Ensure icon-only controls have accessible names and form controls have correct label association.
- Expand Playwright E2E coverage from a single smoke test into a regression suite that asserts:
  - “UI updates immediately” AND “reload remains consistent” for task/project CRUD.
  - No console errors on core routes.

## Capabilities

### New Capabilities
- `task-board-reliability`: Task CRUD + drag/drop interactions update immediately and remain consistent after reload, with clear failure handling.
- `project-management-safety`: Project create/delete flows are explicit and safe in the presence of same-name projects and ambiguous repo paths.
- `frontend-i18n-a11y-baseline`: Touched UI surfaces meet a minimum bar for language consistency and accessible naming/labeling.

### Modified Capabilities
- None.

## Impact

- Frontend:
  - Task streams and mutation flows: `frontend/src/hooks/useJsonPatchWsStream.ts`, `frontend/src/hooks/projects/useProjectTasks.ts`, `frontend/src/hooks/tasks/useAllTasks.ts`, `frontend/src/pages/ProjectTasks.tsx`, `frontend/src/pages/TasksOverview/TasksOverview.tsx`.
  - Kanban column header controls: `frontend/src/components/ui/shadcn-io/kanban/index.tsx`, `frontend/src/components/tasks/TaskKanbanBoard.tsx`.
  - Project create/delete dialogs: `frontend/src/components/dialogs/projects/ProjectFormDialog.tsx`, `frontend/src/components/dialogs/shared/RepoPickerDialog.tsx`, `frontend/src/components/layout/Navbar.tsx`.
- Backend (possible follow-ups depending on findings):
  - Task stream endpoints and event emission for task mutations (`/api/tasks/stream/*`) to ensure updates are broadcast reliably.
- Tests:
  - Extend Playwright suite under `e2e/` and harden the runner (`scripts/run-e2e.js`) for deterministic, repeatable runs.

## Reviewer Guide

- Prioritize P0 reliability: after a mutation resolves successfully, the UI should reflect the new state immediately without navigation hacks.
- Pay special attention to “same-name” and “ambiguous repo path” safety changes in project flows.
- E2E assertions should explicitly cover both “immediate UI update” and “reload consistency” for the same action.

## Goals

- Eliminate the “API succeeded but UI stayed stale” class of failures for task CRUD + drag/drop.
- Restore a functional per-column add-task affordance.
- Reduce catastrophic mis-click risk for project creation/deletion.
- Add E2E coverage that prevents regressions on these core workflows.

## Non-goals

- Redesigning the entire Kanban UI or navigation.
- Introducing a new backend data model for tasks/projects (unless required to fix stream correctness).
- Achieving perfect i18n/a11y coverage across the whole app in one pass (this change focuses on touched surfaces).

## Risks

- Optimistic updates can introduce flicker or temporary inconsistencies if reconciliation rules are unclear.
- Forcing stream resyncs too aggressively could increase load or create jarring UI resets.
- Project creation wizard may add friction; it must remain fast while still being explicit.
- E2E tests can become flaky if stable selectors and deterministic data setup are not addressed.

## Verification

- `pnpm run check` and `pnpm run lint`
- `pnpm run e2e` (expanded suite)
- Manual smoke on: `/tasks`, `/projects/:id/tasks`, `/archives`, `/settings/*` with “mutation → immediate UI update → reload stays consistent”.
