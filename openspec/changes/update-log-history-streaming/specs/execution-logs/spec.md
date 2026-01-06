## ADDED Requirements
### Requirement: Tail-first history retrieval
The system SHALL provide execution log history endpoints that return the latest entries by default and support paging older entries via a cursor.

#### Scenario: Default tail load
- **WHEN** a client requests execution log history without a cursor
- **THEN** the response contains the most recent N entries and indicates whether older history exists

#### Scenario: Load older history
- **WHEN** a client requests history with a cursor
- **THEN** the response returns the next older page and a new cursor when more history is available

### Requirement: Log streams close after finish
The system SHALL send a Finished signal and close live log streams after an execution process finishes.

#### Scenario: Finished terminates stream
- **WHEN** an execution process finishes
- **THEN** the stream emits Finished and no further messages are sent

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
