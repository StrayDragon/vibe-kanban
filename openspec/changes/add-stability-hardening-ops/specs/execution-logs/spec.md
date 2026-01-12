## ADDED Requirements
### Requirement: Lagged stream resync
The system SHALL resynchronize entry-indexed log streams when the underlying broadcast channel reports lag, without closing the stream.

#### Scenario: Lagged receiver resyncs
- **WHEN** a log stream receiver lags behind the broadcast channel
- **THEN** the stream emits a snapshot of current entries with their indexes and continues streaming new events
