## 1. Database & Models

- [ ] 1.1 Add SeaORM migration for archives: create `archived_kanbans` table + add nullable FK `tasks.archived_kanban_id` + indexes (verify: `cargo test -p db-migration` or `cargo check -p db-migration`)
- [ ] 1.2 Add db entities/models for ArchivedKanban (`crates/db/src/entities/archived_kanban.rs`, `crates/db/src/models/archived_kanban.rs`) and wire exports in `crates/db/src/entities/mod.rs` + `crates/db/src/models/mod.rs` (verify: `cargo check -p db`)
- [ ] 1.3 Extend Task DB entity + model to include `archived_kanban_id` and ensure ts-rs type generation includes it on `Task` / `TaskWithAttemptStatus` (verify: `cargo check -p db`)

## 2. Backend HTTP API (Axum)

- [ ] 2.1 Add HTTP routes for archives:
  - `GET /api/projects/:project_id/archived-kanbans` (list)
  - `GET /api/archived-kanbans/:id` (detail/metadata)
  - `POST /api/projects/:project_id/archived-kanbans` (archive-by-status + optional title)
  - `POST /api/archived-kanbans/:id/restore` (restore-all / restore-by-status)
  - `DELETE /api/archived-kanbans/:id` (hard delete archive + tasks)
  (verify: add/extend route tests under `crates/server/src/routes/*` and run `cargo test -p server`)
- [ ] 2.2 Add task listing filters:
  - Extend `/api/tasks` and `/api/tasks/stream/ws` to exclude archived tasks by default
  - Support explicit `include_archived=true` and `archived_kanban_id=<uuid>` filters
  (verify: `cargo test -p server` with a new test that asserts default excludes archived tasks)
- [ ] 2.3 Enforce server-side guardrails for archived tasks:
  - Reject task updates/deletes for `archived_kanban_id != NULL`
  - Reject attempt creation flows for archived tasks (`/api/tasks/create-and-start`, attempt start handlers)
  (verify: `cargo test -p server` covering update/delete/create-and-start rejection)
- [ ] 2.4 Archive/delete pre-check for running processes:
  - Archive MUST reject if any matched task has running execution processes
  - Delete archive MUST reject if any task in the archive has running execution processes
  (verify: `cargo test -p server` with a conflict test)

## 3. MCP Tooling

- [ ] 3.1 Add MCP tools (and schemas) for archived-kanbans:
  - `list_archived_kanbans(project_id)`
  - `archive_project_kanban(project_id, statuses, title?)`
  - `restore_archived_kanban(archive_id, restore_all?, statuses?)`
  - `delete_archived_kanban(archive_id)`
  Ensure `structuredContent` + `outputSchema` and mark destructive tools with `destructiveHint=true` (verify: MCP tool tests in `crates/server/src/mcp/task_server.rs` or adjacent test module)
- [ ] 3.2 Ensure archived tasks are non-executable via MCP pathways as well (verify: add MCP-level test that starting an archived task fails with a structured error)

## 4. Frontend UI

- [ ] 4.1 Add frontend API client for archives (e.g. `frontend/src/api/archived-kanbans.ts`) and export in `frontend/src/api/index.ts` if needed (verify: `pnpm -C frontend run check`)
- [ ] 4.2 Add Project-scoped routes and pages:
  - `/projects/:projectId/archives` (list)
  - `/projects/:projectId/archives/:archiveId` (detail read-only Kanban panel)
  (verify: manual smoke check in dev server)
- [ ] 4.3 Add “Archive” action in Project Kanban view:
  - Dialog: optional title + selectable statuses (default `done`/`cancelled`)
  - Confirmation copy clearly states immutability and non-executability
  (verify: manual smoke check + no TS errors)
- [ ] 4.4 ArchivedKanban detail is fully read-only:
  - No drag/drop, no edits, no create task, no start attempt
  - Provide “Restore” and “Delete archive” actions
  (verify: manual smoke check)
- [ ] 4.5 Implement Restore dialog:
  - restore-all or restore-by-status
  - After restore, tasks reappear in active Kanban without changing status
  (verify: manual smoke check)
- [ ] 4.6 Implement Delete archive flow:
  - High-friction confirmation (typed confirmation recommended)
  - Handle conflict error when running processes exist
  (verify: manual smoke check)
- [ ] 4.7 Update All Tasks view:
  - Default excludes archived tasks
  - Add a toggle to include archived tasks (verify: manual smoke check + `pnpm -C frontend run check`)

## 5. Shared Types (ts-rs)

- [ ] 5.1 Wire new types into `crates/server/src/bin/generate_types.rs` and regenerate `shared/types.ts` via `pnpm run generate-types` (verify: `pnpm -C frontend run check`)

## 6. Test & Verification Pass

- [ ] 6.1 Run backend checks/tests (verify: `pnpm run backend:check` and `cargo test --workspace`)
- [ ] 6.2 Run frontend checks/lint (verify: `pnpm -C frontend run check` and `pnpm -C frontend run lint`)

