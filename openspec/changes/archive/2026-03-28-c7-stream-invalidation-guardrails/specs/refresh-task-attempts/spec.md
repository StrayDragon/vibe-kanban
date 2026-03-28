# refresh-task-attempts Specification (Delta)

## MODIFIED Requirements

### Requirement: SSE-driven task attempt invalidation
The UI SHALL listen to the `/events` SSE stream and invalidate task attempt and branch status queries when relevant updates are received.

The UI MUST prefer backend invalidation hints from `invalidate` events when present, and MUST fall back to parsing `json_patch` events to derive the same invalidations when hints are not available for that update.

To avoid redundant work under bursty realtime updates, the UI MUST batch and deduplicate invalidations within a short window such that repeated events referencing the same identifiers do not trigger repeated invalidation work for the same query keys.

To protect the UI under extreme invalidation bursts, the UI MUST cap the number of unique identifiers processed within a single batch flush. If the UI accumulates more than 512 unique identifiers (sum of unique `taskIds` and unique `workspaceIds`) within the batching window, the UI MUST invalidate all queries and clear the pending batch.

#### Scenario: Invalidate hints refresh workspace attempt queries
- **WHEN** an `invalidate` event includes a workspace id in `workspaceIds`
- **THEN** the UI invalidates branch status and task attempt queries for that workspace id

#### Scenario: Invalidate hints refresh task attempt queries for workspace task_id updates
- **WHEN** an `invalidate` event includes a task id in `taskIds`
- **THEN** the UI invalidates `taskAttempts` and `taskAttemptsWithSessions` queries for that task id

#### Scenario: Json patch fallback still triggers invalidations when hints are absent
- **WHEN** a `json_patch` event contains a workspace add or replace whose `value` includes a `task_id`
- **THEN** the UI invalidates the same task attempt and branch status query keys as the hints-driven flow

#### Scenario: Burst invalidations are batched and deduplicated
- **WHEN** the UI receives multiple `invalidate` events within a short window that reference overlapping task/workspace identifiers
- **THEN** the UI issues at most one invalidation per affected query key for each unique identifier within that batch flush

#### Scenario: Oversized batches degrade to invalidate_all
- **WHEN** the UI receives `invalidate` events within a short window that reference more than 512 unique identifiers across `taskIds` and `workspaceIds`
- **THEN** the UI invalidates all queries at most once and clears the pending invalidation batch

