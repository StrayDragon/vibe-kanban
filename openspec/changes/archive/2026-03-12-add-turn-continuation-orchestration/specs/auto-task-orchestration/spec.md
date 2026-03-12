## ADDED Requirements

### Requirement: Task orchestration reads expose continuation diagnostics
Task orchestration reads SHALL expose continuation counters, effective budgets, and continuation stop reasons when bounded continuation is enabled.

#### Scenario: Client reads continuation diagnostics
- **WHEN** a client reads task detail or task list data for a task that has used continuation turns
- **THEN** the response includes the continuation count, effective budget (and remaining budget where applicable), and the latest continuation stop reason

#### Scenario: Manual task omits continuation diagnostics
- **WHEN** a client reads task detail or task list data for a task whose effective automation mode is manual
- **THEN** continuation diagnostics are absent or null
- **AND** manual task surfaces remain unchanged
