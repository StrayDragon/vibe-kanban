# execution-logs Specification

## Purpose
TBD - created by archiving change update-log-history-streaming. Update Purpose after archive.
## Requirements
### Requirement: Tail-first history retrieval
The system SHALL provide execution log history endpoints for raw and normalized entries that return the latest entries by default and support paging older entries via a cursor.

#### Scenario: Default tail load
- **WHEN** a client requests execution log history without a cursor
- **THEN** the response contains the most recent N entries in chronological order and indicates whether older history exists

#### Scenario: Load older history
- **WHEN** a client requests history with a cursor
- **THEN** the response returns the next older page with stable entry indexes and a new cursor when more history is available

### Requirement: Indexed log history entries
The system MUST assign a monotonically increasing entry index per execution process and include the index in history and stream events.

#### Scenario: Monotonic index
- **WHEN** new log entries are appended
- **THEN** each entry index increases relative to the previous entry

### Requirement: Persistent log entry storage
The system SHALL persist raw and normalized log entries with their indexes so history can be retrieved after an execution process completes.

#### Scenario: Post-completion retrieval
- **WHEN** a client requests history for a completed process
- **THEN** the response is served from persistent storage without relying on in-memory history

### Requirement: Log streams close after finish
The system SHALL send a Finished signal and close live log streams after an execution process finishes.

#### Scenario: Finished terminates stream
- **WHEN** an execution process finishes
- **THEN** the stream emits Finished and no further messages are sent

### Requirement: Entry-indexed live stream events
The system SHALL emit append and replace events that include entry indexes for live streams.

#### Scenario: Append event
- **WHEN** a new log entry arrives during a running process
- **THEN** the stream sends an append event with the entry index and payload

#### Scenario: Replace event
- **WHEN** an existing entry is updated (e.g., tool approval state)
- **THEN** the stream sends a replace event with the same entry index and updated payload

### Requirement: Bounded in-memory log history
The system MUST enforce configured byte and entry limits for in-memory log history per execution.

#### Scenario: Evict oldest history
- **WHEN** incoming log data exceeds the configured limits
- **THEN** the oldest history is evicted while keeping the newest entries

### Requirement: Lazy-load attempt conversation history
The UI SHALL render only the latest conversation entries by default and provide a control to load older history with a loading indicator.

#### Scenario: Default tail view
- **WHEN** an attempt view is opened
- **THEN** the UI shows the latest entries and indicates when older history is available

#### Scenario: Load earlier history
- **WHEN** the user requests older history
- **THEN** the UI fetches and prepends older entries while preserving scroll position

### Requirement: Bounded raw log viewer
The raw log viewer MUST cap the number of retained lines and inform the user when older lines are truncated.

#### Scenario: Raw log cap
- **WHEN** the raw log buffer exceeds the configured limit
- **THEN** older lines are dropped and the UI shows a truncation notice

### Requirement: Evicted history fallback
The system SHALL serve raw and normalized log history from persistent log entry storage when in-memory history has evicted entries for a running process.

#### Scenario: Running process falls back to DB for older entries
- **WHEN** a running process has evicted in-memory history and a client requests history before the earliest in-memory entry
- **THEN** the response returns older entries from persistent storage and indicates whether older history remains

### Requirement: History completeness indicator
The system SHALL indicate when log history is incomplete because older entries were evicted and are not available in persistent storage.

#### Scenario: Missing history flagged
- **WHEN** in-memory history has evicted entries and persistent storage contains no older entries
- **THEN** the history response marks the history as partial for UI display

### Requirement: Lagged stream resync
The system SHALL resynchronize entry-indexed log streams when the underlying broadcast channel reports lag, without closing the stream.

#### Scenario: Lagged receiver resyncs
- **WHEN** a log stream receiver lags behind the broadcast channel
- **THEN** the stream emits a snapshot of current entries with their indexes and continues streaming new events

### Requirement: Legacy log backfill compatibility
The system SHALL backfill indexed raw and normalized log entries from legacy JSONL history when log entry storage is incomplete.

#### Scenario: JSONL-only execution
- **WHEN** a completed execution has JSONL history but missing log entry rows
- **THEN** the system backfills log entries so history endpoints can serve the data

### Requirement: Legacy JSONL retention cleanup
The system SHALL remove legacy JSONL history after a configurable retention window once log entry history is persisted.

#### Scenario: Cleanup after backfill window
- **WHEN** legacy JSONL rows are older than the retention window and the execution is completed
- **THEN** the system deletes the JSONL rows without affecting log entry history retrieval

