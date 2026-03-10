## ADDED Requirements

### Requirement: Task groups store milestone metadata
Each TaskGroup SHALL optionally store milestone metadata fields that describe the goal and default execution preset for its nodes.

Fields:
- `objective` (optional)
- `definitionOfDone` (optional)
- `defaultExecutorProfileId` (optional)
- `automationMode` (required, default `manual`)

#### Scenario: Create task group with milestone fields
- **WHEN** a user creates a task group with objective, definition of done, and a default executor profile
- **THEN** subsequent task group reads return those same fields

#### Scenario: Update milestone metadata
- **WHEN** a user updates the objective, definition of done, default executor profile, or automation mode for a task group
- **THEN** the updated values are persisted and returned by subsequent reads

### Requirement: Milestone identity remains navigable from tasks
Tasks that belong to a task group node SHALL expose enough linkage data for clients to navigate from the task to its owning milestone/task group.

#### Scenario: Navigate from node task to milestone
- **WHEN** a client reads a task that belongs to a task group node
- **THEN** the response includes the owning task group identifier
- **AND** the client can fetch the task group to obtain milestone metadata
