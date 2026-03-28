## MODIFIED Requirements

### Requirement: Backend provides invalidation hints
The backend SHALL provide invalidation hints alongside realtime updates so clients can invalidate caches without parsing JSON Pointer paths.

For WebSocket JSON-Patch messages (messages that contain a `JsonPatch` field), when the patch includes operations targeting entity paths under:
- `/tasks/<task_id>`
- `/workspaces/<workspace_id>`
- `/execution_processes/<process_id>`

the backend MUST include an `invalidate` field.

When present, `invalidate` MUST be a JSON object with the following shape:
- `taskIds`: array of string UUIDs (possibly empty)
- `workspaceIds`: array of string UUIDs (possibly empty)
- `hasExecutionProcess`: boolean

#### Scenario: Hints have a stable schema
- **WHEN** a realtime WS JSON-Patch message includes an `invalidate` field
- **THEN** `invalidate.taskIds` is an array of strings
- **AND** `invalidate.workspaceIds` is an array of strings
- **AND** `invalidate.hasExecutionProcess` is a boolean

#### Scenario: Task entity updates include the affected task id
- **WHEN** a realtime WS JSON-Patch message contains an operation with a path under `/tasks/<task_id>`
- **THEN** `invalidate.taskIds` includes `<task_id>`

#### Scenario: Workspace entity updates include workspace id and related task id
- **WHEN** a realtime WS JSON-Patch message contains an add/replace operation with a path under `/workspaces/<workspace_id>`
- **AND** the operation value contains a `task_id` field
- **THEN** `invalidate.workspaceIds` includes `<workspace_id>`
- **AND** `invalidate.taskIds` includes that `task_id`

#### Scenario: Execution process updates set the execution process flag
- **WHEN** a realtime WS JSON-Patch message contains any operation with a path under `/execution_processes/<process_id>`
- **THEN** `invalidate.hasExecutionProcess` is `true`

