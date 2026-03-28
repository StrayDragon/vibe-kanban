# execution-logs Specification (Delta)

## MODIFIED Requirements

### Requirement: LogMsg streams resynchronize on lag
The system SHALL resynchronize realtime `LogMsg` streams when the underlying broadcast channel reports lag, without silently dropping messages, as long as the missed window is still retained.

When resynchronizing within the retained window, the server MUST replay only messages whose `seq` is greater than the receiver's last observed `last_seq` and MUST NOT emit an unnecessary full-history snapshot.

#### Scenario: Lagged receiver replays from retained history
- **WHEN** a `LogMsg` stream receiver lags and the missed `seq` range is still retained in server history
- **THEN** the stream replays the missed messages with `seq > last_seq` in order and continues streaming new messages
- **AND** the server logs a warning that includes `skipped`, `last_seq`, and the current retained `max_seq`

#### Scenario: Lag beyond retained window degrades explicitly
- **WHEN** a receiver lags beyond the retained history window
- **THEN** the stream continues from the newest retained message and the gap is detectable via `seq` semantics
- **AND** the server logs a warning that the gap exceeded retained history and includes `last_seq` plus the retained `min_seq`/`max_seq`

