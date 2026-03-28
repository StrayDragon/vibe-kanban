## ADDED Requirements

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

