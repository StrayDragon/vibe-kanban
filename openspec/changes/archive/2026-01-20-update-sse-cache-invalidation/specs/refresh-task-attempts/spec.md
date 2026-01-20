## ADDED Requirements
### Requirement: SSE-driven task attempt invalidation
The UI SHALL listen to the `/events` SSE stream and invalidate task attempt and branch status queries when relevant JSON Patch events are received.

#### Scenario: Workspace patch invalidates attempts
- **WHEN** a `json_patch` event contains a workspace add/replace with a `task_id`
- **THEN** the UI invalidates `taskAttempts` and `taskAttemptsWithSessions` for that task and invalidates `branchStatus` for that workspace

#### Scenario: Execution process patch invalidates branch status
- **WHEN** a `json_patch` event contains an execution process add/replace/remove
- **THEN** the UI invalidates branch status queries to refresh ahead/behind state

### Requirement: Visibility-aware fallback polling
The UI SHALL poll task attempt and branch status queries only when the SSE stream is disconnected and the document is visible.

#### Scenario: SSE connected
- **WHEN** the SSE connection is open
- **THEN** periodic polling for task attempts and branch status is disabled

#### Scenario: SSE disconnected while visible
- **WHEN** the SSE connection is closed and the document is visible
- **THEN** the UI polls those queries at the configured fallback interval
