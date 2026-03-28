# execution-logs Specification (Delta)

## ADDED Requirements

### Requirement: Log broadcast capacity is configurable
The system SHALL allow configuring the capacity of realtime in-memory log broadcast buffers so operators can trade off memory usage vs. lag tolerance.

#### Scenario: Capacity is configured
- **WHEN** `VK_LOG_BROADCAST_CAPACITY` is set to a positive integer
- **THEN** the server uses that capacity for execution-process log broadcast channels

#### Scenario: Invalid capacity falls back
- **WHEN** `VK_LOG_BROADCAST_CAPACITY` is unset, zero, or invalid
- **THEN** the server falls back to a safe default capacity and logs a warning

### Requirement: LogMsg streams resynchronize on lag
The system SHALL resynchronize realtime `LogMsg` streams when the underlying broadcast channel reports lag, without silently dropping messages, as long as the missed window is still retained.

#### Scenario: Lagged receiver replays from retained history
- **WHEN** a `LogMsg` stream receiver lags and the missed `seq` range is still retained in server history
- **THEN** the stream replays the missed messages in order and continues streaming new messages

#### Scenario: Lag beyond retained window degrades explicitly
- **WHEN** a receiver lags beyond the retained history window
- **THEN** the stream continues from the newest retained message and the gap is detectable via `seq` semantics (or logged by the server)
