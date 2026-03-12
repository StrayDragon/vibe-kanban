## Definitions

- **Auto-managed task**: a milestone node task inside a milestone with `automation_mode=auto`, where the task has a non-empty `milestone_node_id` and is not the milestone entry task itself.

## ADDED Requirements

### Requirement: Auto-managed scope is explicit and narrow
The system SHALL only expose "auto-managed" orchestration collaboration diagnostics for tasks that match the auto-managed definition above.

#### Scenario: Non-managed task does not emit auto-managed diagnostics
- **WHEN** a client reads task detail for a task that is not auto-managed
- **THEN** the response does not include misleading orchestration collaboration diagnostics for that task

### Requirement: Task orchestration reads expose collaboration diagnostics
Task orchestration reads SHALL expose the collaboration diagnostics needed by both human and MCP clients.

#### Scenario: Task detail includes transfer and policy diagnostics
- **WHEN** a client reads task detail for an auto-managed task
- **THEN** the response includes any active control-transfer reason and any active executor policy diagnostic
- **AND** those diagnostics remain consistent with the task's effective automation mode
