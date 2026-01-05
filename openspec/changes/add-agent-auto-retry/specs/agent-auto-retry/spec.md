## ADDED Requirements
### Requirement: Auto-Retry Configuration
The system SHALL allow per-executor configuration of auto-retry behavior, including a list of recoverable error regex patterns, a retry delay in seconds, and a maximum retry attempt count.

#### Scenario: Configure auto-retry for an executor
- **WHEN** a user edits an executor configuration in `/settings/agents`
- **THEN** they can set `error_patterns`, `delay_seconds`, and `max_attempts`

### Requirement: Auto-Retry Triggering
The system SHALL automatically retry failed coding-agent executions when the error output matches a configured regex pattern, waiting the configured delay and honoring the configured maximum attempts.

#### Scenario: Matched recoverable error triggers retry
- **WHEN** a coding-agent execution finishes with status `failed`
- **AND** the error output matches one of the configured regex patterns
- **AND** the retry attempt count is below `max_attempts`
- **THEN** the system schedules a new retry after `delay_seconds`

### Requirement: Auto-Retry Tip Display
The system SHALL display a system tip in the conversation when an auto-retry is scheduled or executed.

#### Scenario: Tip shown after auto-retry scheduling
- **WHEN** the system schedules an auto-retry
- **THEN** a light-green system tip appears indicating the retry delay and attempt count
