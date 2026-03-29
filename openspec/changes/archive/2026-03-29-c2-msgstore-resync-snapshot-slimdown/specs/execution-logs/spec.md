# execution-logs Specification (Delta)

## ADDED Requirements

### Requirement: Lag resync snapshot emission avoids duplicate buffering
When resynchronizing entry-indexed log streams after a broadcast lag, the system SHALL emit snapshot Replace events without constructing a second full in-memory event buffer that duplicates the snapshot contents.

#### Scenario: Lagged receiver resync does not double-buffer snapshot
- **WHEN** a log entry stream receiver lags and the server performs a snapshot resync
- **THEN** the server emits Replace events derived from the snapshot
- **AND** the resync implementation does not allocate an additional full in-memory queue that duplicates the snapshot entries
