# execution-logs Specification (Delta)

## MODIFIED Requirements

### Requirement: Lagged stream resync
The system SHALL resynchronize entry-indexed log streams when the underlying broadcast channel reports lag, without closing the stream.

#### Scenario: Lagged receiver resyncs
- **WHEN** a log stream receiver lags behind the broadcast channel
- **THEN** the stream emits a snapshot of current entries with their indexes and continues streaming new events
- **AND** the server logs a warning that includes the lagged skipped count and indicates that a snapshot resync occurred

### Requirement: LogMsg streams resynchronize on lag
The system SHALL resynchronize realtime `LogMsg` streams when the underlying broadcast channel reports lag, without silently dropping messages, as long as the missed window is still retained.

#### Scenario: Lagged receiver replays from retained history
- **WHEN** a `LogMsg` stream receiver lags and the missed `seq` range is still retained in server history
- **THEN** the stream replays the missed messages in order and continues streaming new messages
- **AND** the server logs a warning that includes `skipped`, `last_seq`, and the current retained `max_seq`

#### Scenario: Lag beyond retained window degrades explicitly
- **WHEN** a receiver lags beyond the retained history window
- **THEN** the stream continues from the newest retained message and the gap is detectable via `seq` semantics
- **AND** the server logs a warning that the gap exceeded retained history and includes `last_seq` plus the retained `min_seq`/`max_seq`
