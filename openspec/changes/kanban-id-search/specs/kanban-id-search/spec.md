## ADDED Requirements

### Requirement: Tasks expose a stable number and short ID

The system SHALL expose:

- `Task.number`: a stable numeric identifier
- `Task.short_id`: a deterministic short identifier derived from `Task.id`

These fields MUST be present in task responses used by the kanban UI.

#### Scenario: Task data includes identifiers

- **WHEN** the client loads tasks for a project kanban
- **THEN** each task includes `number` and `short_id`

### Requirement: Kanban search matches by number and short ID

The kanban search filter SHALL match tasks by:

- `#<number>` or `<number>` (exact match)
- `short_id` or UUID prefix (case-insensitive match)
- title/description (existing behavior)

#### Scenario: Search by number

- **WHEN** the user enters `#123` (or `123`) into the kanban search input
- **THEN** only the task with `number = 123` remains visible

#### Scenario: Search by short ID

- **WHEN** the user enters a task's `short_id` into the kanban search input
- **THEN** that task remains visible

