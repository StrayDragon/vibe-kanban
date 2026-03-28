## ADDED Requirements

### Requirement: Snapshot patches replace full entity maps
Realtime WS streams that represent entity maps (e.g. tasks, projects, execution processes) SHALL emit snapshots as a single JSON Patch operation that replaces the full map.

The snapshot patch MUST:
- contain exactly one operation
- use `op: "replace"`
- use `path: "/<mapKey>"`
- set `value` to a JSON object mapping string ids to entity objects

#### Scenario: Tasks stream snapshot replaces full tasks map
- **WHEN** a client connects to the tasks realtime stream and the server cannot resume from `after_seq`
- **THEN** the server emits a `JsonPatch` message containing exactly one operation with `op: "replace"` and `path: "/tasks"`

#### Scenario: Execution processes stream snapshot replaces full execution processes map
- **WHEN** a client connects to the execution processes realtime stream and the server cannot resume from `after_seq`
- **THEN** the server emits a `JsonPatch` message containing exactly one operation with `op: "replace"` and `path: "/execution_processes"`

### Requirement: Delta patches are single-operation entity updates
Realtime WS streams that represent entity maps MUST emit delta updates as a JSON Patch containing exactly one operation that targets a single entity id path.

The delta patch MUST:
- contain exactly one operation
- use `path: "/<mapKey>/<id>"`
- use `op` in `{ "add", "replace", "remove" }`

#### Scenario: Delta patch updates one task entity
- **WHEN** the server emits a delta update for a single task on the tasks realtime stream
- **THEN** the message `JsonPatch` contains exactly one operation whose `path` starts with `/tasks/`

#### Scenario: Delta patch updates one execution process entity
- **WHEN** the server emits a delta update for a single execution process on the execution processes realtime stream
- **THEN** the message `JsonPatch` contains exactly one operation whose `path` starts with `/execution_processes/`

### Requirement: Filtered tasks stream enforces view via remove translation
When the tasks realtime stream is subscribed with filtering (by project and/or archived selection), the server SHALL ensure that non-matching entity updates do not remain visible to the client.

For add/replace patches under `/tasks/<task_id>`:
- If the task matches the subscriber's filter, the server MUST forward the original patch.
- If the task does NOT match the subscriber's filter, the server MUST emit a remove patch for the same `/tasks/<task_id>` path.

For remove patches under `/tasks/<task_id>`:
- The server MUST forward the original remove patch.

#### Scenario: Non-matching task add is translated to remove
- **WHEN** the server receives or replays an add patch under `/tasks/<task_id>` for a task that does not match the subscriber filter
- **THEN** the server emits a `JsonPatch` containing exactly one `remove` operation for `/tasks/<task_id>`

#### Scenario: Matching task replace is forwarded
- **WHEN** the server receives or replays a replace patch under `/tasks/<task_id>` for a task that matches the subscriber filter
- **THEN** the server forwards the original patch unchanged

### Requirement: Dropped execution processes are hidden when show_soft_deleted is false
When streaming execution processes for a workspace with `show_soft_deleted` set to `false`, the server MUST NOT forward add/replace patches for dropped processes.

For add/replace patches under `/execution_processes/<process_id>`:
- If the process is marked as dropped, the server MUST emit a remove patch for `/execution_processes/<process_id>` instead of forwarding the add/replace patch.

For remove patches under `/execution_processes/<process_id>`:
- The server MUST forward the original remove patch.

#### Scenario: Dropped process add becomes remove when show_soft_deleted is false
- **WHEN** the server processes an add patch under `/execution_processes/<process_id>` whose value indicates the process is dropped
- **AND** `show_soft_deleted` is `false`
- **THEN** the server emits a `JsonPatch` containing exactly one `remove` operation for `/execution_processes/<process_id>`

#### Scenario: Dropped process patch is forwarded when show_soft_deleted is true
- **WHEN** the server processes an add or replace patch under `/execution_processes/<process_id>` whose value indicates the process is dropped
- **AND** `show_soft_deleted` is `true`
- **THEN** the server forwards the original patch unchanged

