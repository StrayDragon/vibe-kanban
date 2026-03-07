## ADDED Requirements

### Requirement: Task orchestration reads expose collaboration diagnostics
Task orchestration reads SHALL expose the collaboration diagnostics needed by both human and MCP clients.

#### Scenario: Task detail includes transfer and policy diagnostics
- **WHEN** a client reads task detail for an auto-managed task
- **THEN** the response includes any active control-transfer reason and any active executor policy diagnostic
- **AND** those diagnostics remain consistent with the task's effective automation mode
