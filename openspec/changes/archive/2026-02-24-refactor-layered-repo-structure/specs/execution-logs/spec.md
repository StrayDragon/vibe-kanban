## ADDED Requirements

### Requirement: Execution logs endpoints remain stable
The system SHALL preserve existing `/api/execution-processes/*` log endpoints while internal code is split into layered modules.

#### Scenario: Logs stream remains functional
- **WHEN** a client subscribes to the existing log WebSocket endpoint
- **THEN** the connection succeeds and events are emitted in the expected format
