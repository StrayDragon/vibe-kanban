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

