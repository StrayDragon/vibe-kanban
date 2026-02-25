## ADDED Requirements
### Requirement: Non-blocking git operations in request paths
The system SHALL execute CLI-backed git operations invoked by HTTP request handlers without blocking Tokio worker threads.

#### Scenario: Merge request path remains responsive
- **WHEN** a merge-related API request triggers git CLI commands
- **THEN** the commands run in a blocking-safe execution boundary and do not block async workers

### Requirement: Stable initial conversation load
The UI SHALL load initial conversation history at most once per attempt lifecycle and SHALL not reset displayed entries on routine stream updates.

#### Scenario: Stream update after initial load
- **WHEN** live execution-process updates arrive after initial history load
- **THEN** the displayed history is incrementally updated without replaying initial-load reset behavior

### Requirement: Follow-up UI modular decomposition
The follow-up interaction UI SHALL preserve current user-visible behavior after decomposition into smaller components or hooks.

#### Scenario: Send follow-up after refactor
- **WHEN** a user sends a follow-up message with existing options
- **THEN** the API request payload and resulting UI state remain behaviorally equivalent
