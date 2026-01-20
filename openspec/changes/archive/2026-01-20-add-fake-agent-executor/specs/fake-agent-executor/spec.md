## ADDED Requirements
### Requirement: Fake agent availability
The system SHALL provide a Fake agent executor that can be selected in executor profiles and used in all runtime environments.

#### Scenario: Select Fake agent
- **WHEN** a user selects the Fake agent executor in settings
- **THEN** new task attempts run using the Fake agent executor

### Requirement: Codex-compatible streaming
The Fake agent SHALL emit Codex-compatible JSONL events, including a session identifier and streaming assistant message deltas, so the existing log normalization and UI streams are exercised.

#### Scenario: Stream assistant output
- **WHEN** a Fake agent run starts
- **THEN** the log stream emits a session configuration event and streaming assistant deltas until completion

### Requirement: Deterministic simulation controls
The Fake agent SHALL support a configurable seed and timing parameters so repeated runs can generate deterministic event sequences.

#### Scenario: Repeatable output
- **WHEN** a Fake agent run is executed twice with the same seed and config
- **THEN** the emitted event sequence is identical in order and content

### Requirement: Safety by default
The Fake agent MUST NOT execute real filesystem or network operations and MUST only simulate tool events.

#### Scenario: Safe execution
- **WHEN** the Fake agent emits tool events
- **THEN** no external command or file modification is performed
