## ADDED Requirements
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
