## ADDED Requirements

### Requirement: Orchestration behavior unchanged by refactors
The system SHALL preserve existing orchestration behavior (task → attempt → session → execution process) while internal services are reorganized.

#### Scenario: Create-and-start flow still works
- **WHEN** a client calls the existing create-and-start endpoint
- **THEN** a task is created and an attempt is started as before (same observable results)
