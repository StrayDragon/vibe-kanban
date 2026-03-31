## ADDED Requirements

### Requirement: Kanban create buttons open the create dialog directly
In the Kanban UI, create-entry buttons (for example “+”) SHALL open the create dialog directly and SHALL NOT require selecting from a secondary menu.

#### Scenario: Plus click opens create dialog
- **WHEN** an operator clicks a Kanban create “+” button
- **THEN** the system opens the create dialog immediately
- **AND** no intermediate menu is shown

### Requirement: The create dialog supports creating both tasks and task groups
The create dialog SHALL support creating either a Task or a Task Group using a consistent, unified UI.

#### Scenario: Operator creates a task from the dialog
- **WHEN** the operator selects “Task” in the create dialog and submits valid fields
- **THEN** a new task is created under the selected project/context

#### Scenario: Operator creates a task group from the dialog
- **WHEN** the operator selects “Task Group” in the create dialog and submits valid fields
- **THEN** a new task group is created under the selected project/context

### Requirement: Default create kind is Task when opened from Kanban “+”
When opened from a Kanban “+” entry point, the create dialog SHALL default to creating a Task.

#### Scenario: Default kind is Task
- **WHEN** the operator opens the create dialog from a Kanban “+” button
- **THEN** the dialog defaults to “Task” without requiring additional clicks

