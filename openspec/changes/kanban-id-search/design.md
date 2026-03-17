## Context

The Project Kanban page (`frontend/src/pages/ProjectTasks.tsx`) currently filters
tasks in-memory based on title/description only. The backend task model exposes
UUID `id` but does not expose the stable numeric primary key from the DB, and it
does not provide a short ID.

## Goals / Non-Goals

**Goals:**
- Provide a stable `number` (numeric identifier) and `short_id` for tasks.
- Update kanban search matching to include `number` and `short_id` (and UUID
  prefix) in addition to title/description.
- Display `#<number>` in task cards for easy referencing.

**Non-Goals:**
- No server-side search endpoint changes; this remains a client-side filter over
  the streamed task list.
- No per-project numbering scheme in this change.

## Decisions

### Decision: Use the existing DB primary key as `Task.number`

The `tasks` table already has an integer primary key (`tasks.id`). We will
expose this as `Task.number` in API responses. This value is stable and requires
no new schema changes.

### Decision: Define `Task.short_id` as the UUID prefix

We will define `short_id` as the first 8 characters of the task UUID in
lowercase. This keeps the ID deterministic without adding storage.

### Decision: Search matching rules

Given a user search query `q` (trimmed):

1. If `q` matches `^#?\\d+$`, treat it as a number search and match tasks where
   `task.number == parsed(q)`.
2. Otherwise, match tasks where:
   - `task.short_id` contains `q` (case-insensitive), OR
   - `task.id` (UUID string) contains `q` (case-insensitive), OR
   - title/description contains `q` (existing behavior)

This keeps behavior predictable and avoids surprising partial numeric matches.

### Decision: UI display

In the kanban card and task detail header, display `#<number>` in a small badge.
We will not display `short_id` by default (search-only), to avoid visual noise.

## Risks / Trade-offs

- **[Number semantics]** global vs per-project numbering → use global numeric ID
  now; revisit per-project sequences only if requested.
- **[UX clutter]** → show only `#<number>` by default.

## Migration Plan

1. Backend: Add `number` and `short_id` to task DTOs and regenerate TS types.
2. Frontend: Update kanban search matching to include these fields.
3. Frontend: Display `#<number>` in task cards.

## Open Questions

- Should we also accept `T-123` style prefixes? (Default: **no**, only `#123`
  and `123`.)
- Should number matching support partial matches? (Default: **no**, exact
  number match.)

