# task-list-hydration Specification

## Purpose
TBD - created by archiving change c3-backend-task-hydration-de-nplus1. Update Purpose after archive.
## Requirements
### Requirement: Task list hydration is batched and stable
The backend SHALL hydrate task lists (for example `TaskWithAttemptStatus`) using batched database reads, avoiding per-task fan-out queries for:
- foreign-key UUID resolution
- attempt status summaries
- dispatch state lookups
- auto-orchestration diagnostics lookups

This requirement applies to task list API responses and realtime tasks snapshot/resync builders that rely on the same hydration logic.

#### Scenario: Listing tasks returns attempt status summaries
- **WHEN** a client requests a list of tasks
- **THEN** each task includes `has_in_progress_attempt` and `last_attempt_failed`
- **AND** each task includes an `executor` string (empty when unknown)

#### Scenario: Listing tasks includes dispatch and orchestration fields when present
- **WHEN** a listed task has a dispatch state record and/or orchestration state
- **THEN** the task payload includes `dispatch_state` and/or `orchestration` for that task

### Requirement: Archived kanban task filtering is correct
When listing tasks for a specific archived kanban, the backend SHALL filter by the archived kanban identifier and SHALL include `archived_kanban_id` in the returned task payloads.

#### Scenario: Filter by archived kanban id
- **WHEN** a client requests tasks with `archived_kanban_id = <id>`
- **THEN** only tasks belonging to that archived kanban are returned
- **AND** each returned task includes `archived_kanban_id = <id>`

