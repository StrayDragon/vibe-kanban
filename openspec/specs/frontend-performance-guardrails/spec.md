# frontend-performance-guardrails Specification

## Purpose
TBD - created by archiving change frontend-optimization-phase-5-performance-and-query-keys. Update Purpose after archive.
## Requirements
### Requirement: Route-level code splitting prevents a single large entry chunk
The frontend router SHALL lazy-load non-root route modules to avoid bundling all pages into a single entry chunk.

#### Scenario: Production build emits code-split chunks
- **WHEN** a developer runs `pnpm -C frontend run build`
- **THEN** the output contains code-split JavaScript chunks (more than one JS bundle under `frontend/dist/assets/`)

### Requirement: Archived kanban task derivations are incremental and stable
When consuming realtime task streams, the frontend SHALL avoid full rebuilds of derived collections when updates are localized to a small set of task ids.

For archived kanban task views derived from a `tasksById` map:
- `tasks` and `tasksByStatus` MUST be sorted by `created_at` descending (tie-break by `id`).
- For id-local updates (add/replace/remove at `/tasks/<id>`), the frontend MUST apply incremental derivation updates keyed by the invalidation task ids, rather than re-sorting all tasks on every patch.
- Unaffected per-status lists SHOULD preserve referential equality across localized updates to maximize React memoization efficiency.

#### Scenario: Derived task lists remain correctly sorted
- **WHEN** the archived kanban task view receives a sequence of localized task updates
- **THEN** the derived `tasks` list remains sorted by `created_at` descending (tie-break by `id`)
- **AND** each `tasksByStatus[status]` list remains sorted by the same ordering

#### Scenario: Unaffected status lists keep stable references on localized updates
- **WHEN** a localized update modifies a task within a single status without moving it across statuses
- **THEN** the derived list for that status may change
- **AND** the derived lists for other statuses remain referentially stable

### Requirement: Conversation history derivations are incremental and stable
When consuming realtime execution-process updates, the frontend SHALL avoid full rebuilds of derived conversation history collections when updates are localized to a small set of execution process ids.

For conversation history derived from entry-indexed log pages:
- Per-process entry lists MUST remain sorted by `entry_index` ascending.
- For id-local updates affecting a single execution process, the frontend MUST apply incremental derivation updates scoped to that process, rather than re-sorting and re-flattening all processes on every update.
- Unaffected per-process entry lists SHOULD preserve referential equality across localized updates to maximize React memoization efficiency.

#### Scenario: Derived conversation entries remain correctly ordered
- **WHEN** a conversation view receives a sequence of localized execution-process entry updates
- **THEN** per-process entries remain sorted by `entry_index` ascending

#### Scenario: Unaffected process lists keep stable references on localized updates
- **WHEN** a localized update modifies entries for one execution process
- **THEN** the derived entry lists for other execution processes remain referentially stable

