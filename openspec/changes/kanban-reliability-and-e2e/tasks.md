## 1. Task board reliability (P0)

- [x] 1.1 Fix Kanban column add-task button hitbox/styling and add a stable selector (e.g., `data-testid`). Verification: manual click in `/projects/:id/tasks` + Playwright can `getByRole('button', { name: 'Add task' })`.
- [x] 1.2 Add optimistic drag-and-drop status updates with rollback + toast on failure. Verification: drag a card across columns, observe immediate move, force an API failure and confirm rollback.
- [x] 1.3 Make delete remove cards immediately (and prevent “ghost cards” after reload), with rollback + toast on failure. Verification: delete a card, confirm it disappears immediately and stays gone after `page.reload()`.
- [x] 1.4 Add a non-navigation resync mechanism for task streams (soft reconnect that preserves UI while resyncing). Verification: simulate WS disconnect/staleness and confirm the board can recover without leaving the route.
- [x] 1.5 Audit backend task stream emission for create/update/delete/status-change and patch any missing broadcasts. Verification: manual repro no longer requires “switch away and back” + E2E mutation test passes reliably.

## 2. Project management safety (P0)

- [x] 2.1 Refactor `ProjectFormDialog` into an explicit wizard (choose repo → confirm name → explicit create). Verification: selecting a repo alone does NOT create a project; only explicit confirm does.
- [x] 2.2 Make repo selection UI side-effect free (no “click list item → creates project”), using real button/radio semantics. Verification: keyboard navigation selects repo without creating project until confirm.
- [x] 2.3 Add unsafe-path heuristics (worktree/temp directories) and block or require explicit acknowledgement before creating. Verification: selecting a worktree-like path triggers a warning/ack step.
- [x] 2.4 Disambiguate same-name projects in selection + delete confirmations by showing repo path and/or IDs. Verification: create two same-name projects and confirm UI + delete dialog remain unambiguous.

## 3. i18n + accessibility baseline (P1)

- [x] 3.1 Remove hard-coded strings on touched surfaces (dialogs/buttons/empty states) and ensure language switch applies without refresh. Verification: switch language in Settings and see updated labels immediately.
- [x] 3.2 Ensure icon-only controls have accessible names and form controls have correct label association on touched surfaces. Verification: Playwright locators can use `getByRole(..., { name: ... })` / `getByLabel(...)` and succeed.
- [x] 3.3 Fix any ARIA warnings surfaced during implementation (e.g., invalid attributes) on the affected routes. Verification: no accessibility-related console warnings during manual smoke on `/tasks`, `/projects/:id/tasks`, `/settings/*`.

## 4. Playwright E2E regression suite (P0/P1)

- [x] 4.1 Add reusable E2E helpers/fixtures for creating repos/projects/tasks deterministically (seeded names, serial mode where needed). Verification: `pnpm run e2e` passes twice in a row without manual cleanup.
- [x] 4.2 Add E2E coverage for `/projects/:id/tasks`: create/edit/delete/drag and assert “immediate UI update + reload consistency”. Verification: `pnpm run e2e:test -- --grep \"project tasks\"`.
- [x] 4.3 Add E2E coverage for `/tasks`: grouping, collapse/expand, filters/inbox, and “reload consistency”. Verification: `pnpm run e2e:test -- --grep \"tasks overview\"`.
- [x] 4.4 Add E2E coverage for project wizard safety + delete confirmation disambiguation. Verification: `pnpm run e2e:test -- --grep \"project safety\"`.
- [x] 4.5 Add E2E coverage for external links (Docs/Support) opening in a new tab without blocking the app. Verification: `pnpm run e2e:test -- --grep \"external links\"`.
- [x] 4.6 Add E2E coverage for `/settings/*` deep links and language persistence across reload. Verification: `pnpm run e2e:test -- --grep \"settings\"`.
