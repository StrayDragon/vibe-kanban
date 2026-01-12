## ADDED Requirements
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
