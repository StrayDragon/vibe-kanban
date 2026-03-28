# realtime-stream-resilience Specification

## Purpose
TBD - created by archiving change frontend-hardening-phase-3-stream-protocol-resilience. Update Purpose after archive.
## Requirements
### Requirement: Realtime patch messages are sequenced
Realtime patch messages delivered over WebSocket SHALL include a monotonic `seq` value that allows clients to detect message gaps.

#### Scenario: Patch messages include seq
- **WHEN** a client connects to a realtime WS JSON-Patch stream
- **THEN** each JSON-Patch message includes a numeric `seq` field

#### Scenario: Seq is monotonic
- **WHEN** the server emits successive messages on a given stream
- **THEN** the `seq` values strictly increase over time

### Requirement: WebSocket streams support short-window resume
Realtime WS streams SHALL support resuming from a recent point using an `after_seq` parameter when the server still retains history for that window.

#### Scenario: Resume within buffer replays missed messages
- **WHEN** a client reconnects with `after_seq` equal to its last observed `seq`
- **THEN** the server replays messages with `seq` greater than `after_seq` without requiring a full snapshot

#### Scenario: Resume outside buffer triggers resync snapshot
- **WHEN** a client reconnects with `after_seq` older than the server’s retained minimum
- **THEN** the server provides a full snapshot so the client can resync to canonical state

### Requirement: Backend provides invalidation hints
The backend SHALL provide invalidation hints alongside realtime updates so clients can invalidate caches without parsing JSON Pointer paths.

#### Scenario: Hints are present for entity updates
- **WHEN** a realtime patch modifies tasks/workspaces/execution processes
- **THEN** the message includes an `invalidate` hint payload containing the affected identifiers or flags

### Requirement: Protocol changes are backward compatible
Sequencing and hints SHALL be additive and MUST NOT break clients that only recognize legacy fields.

#### Scenario: Legacy clients still apply patches
- **WHEN** a legacy client receives a message containing extra fields (e.g., `seq`, `invalidate`)
- **THEN** it can still process messages based on existing `JsonPatch` or `finished` fields

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

### Requirement: SSE emits at most one event per sequenced patch update
The `/events` SSE stream SHALL emit at most one SSE event per sequenced JSON Patch update.

When the backend can derive invalidation hints for a sequenced JSON Patch update, the server MUST emit an `invalidate` SSE event with `id` equal to that update's `seq`, and MUST NOT also emit a `json_patch` SSE event with the same `id`.

When invalidation hints are not available for a sequenced JSON Patch update, the server MUST emit a `json_patch` SSE event with `id` equal to that update's `seq`.

#### Scenario: Hints available emits only invalidate
- **WHEN** a sequenced JSON Patch update has derivable invalidation hints
- **THEN** the server emits a single `invalidate` SSE event for that update's `seq`
- **AND** the server does not emit a `json_patch` SSE event with the same `id`

#### Scenario: Hints unavailable emits only json_patch
- **WHEN** a sequenced JSON Patch update does not have derivable invalidation hints
- **THEN** the server emits a single `json_patch` SSE event for that update's `seq`

### Requirement: SSE invalidate_all uses watermark identifiers
When the server cannot guarantee SSE stream continuity for a client, it SHALL emit an `invalidate_all` SSE event with `id` equal to the current stream watermark.

The `invalidate_all` payload MUST be valid JSON and MUST include:
- `reason`
- `watermark`

For `reason: "resume_unavailable"`, the payload MUST include:
- `requested_after_seq`
- `min_seq`
- `evicted`

For `reason: "lagged"`, the payload MUST include:
- `skipped`

#### Scenario: Resume unavailable emits invalidate_all with watermark id
- **WHEN** a client attempts to resume the SSE stream from an unavailable `after_seq`
- **THEN** the server emits an `invalidate_all` SSE event
- **AND** the event `id` equals the current `watermark`
- **AND** the payload includes `reason: "resume_unavailable"` and the required fields

#### Scenario: Lagged receiver emits invalidate_all with watermark id
- **WHEN** a client falls behind and the server drops SSE messages (lagged receiver)
- **THEN** the server emits an `invalidate_all` SSE event
- **AND** the event `id` equals the current `watermark`
- **AND** the payload includes `reason: "lagged"` and the required fields

