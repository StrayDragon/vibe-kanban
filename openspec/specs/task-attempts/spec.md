# task-attempts Specification

## Purpose
TBD - created by archiving change add-task-attempts-with-latest-session. Update Purpose after archive.
## Requirements
### Requirement: Task attempts with latest session API
The system SHALL provide an API endpoint that returns task attempts with the latest session per attempt in a single response.

#### Scenario: Fetch attempts with latest sessions
- **WHEN** a client requests task attempts for a task
- **THEN** the response includes each task attempt with its latest session data

