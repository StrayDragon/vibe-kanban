## ADDED Requirements

### Requirement: Config endpoints remain stable
The system SHALL preserve existing config endpoints and response shapes while server-side modules are reorganized.

#### Scenario: Settings UI continues to load
- **WHEN** the frontend requests `/api/info`
- **THEN** the response still contains the expected `config` payload and required metadata
