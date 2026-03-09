## ADDED Requirements

### Requirement: Milestones define an objective and definition of done
The system SHALL allow a Milestone to store an objective and a definition of done that describe the desired end state in human- and agent-readable form.

#### Scenario: Create milestone with objective
- **WHEN** a user creates a milestone with an objective and definition of done
- **THEN** the milestone is persisted with those values
- **AND** subsequent milestone reads return the same values

#### Scenario: Update milestone objective
- **WHEN** a user updates the milestone objective or definition of done
- **THEN** the updated values are persisted and returned by subsequent reads

### Requirement: Milestones provide execution presets
Each milestone SHALL support a default executor profile preset used for node tasks that do not specify an explicit executor profile.

#### Scenario: Node inherits milestone preset
- **WHEN** a milestone defines a default executor profile
- **AND** a node does not define an executor profile override
- **AND** an attempt is started for that node
- **THEN** the attempt uses the milestone's default executor profile

#### Scenario: Node override takes precedence
- **WHEN** a node defines an executor profile override
- **AND** the milestone also defines a default executor profile
- **THEN** attempts started for that node use the node override

### Requirement: Milestones support bounded automation mode
Each milestone SHALL expose an automation mode that controls whether milestone node tasks are eligible for unattended dispatch.

#### Scenario: Milestone automation disabled
- **WHEN** a milestone automation mode is disabled
- **THEN** the scheduler SHALL NOT dispatch node tasks solely due to milestone membership

#### Scenario: Milestone automation enabled
- **WHEN** a milestone automation mode is enabled
- **THEN** eligible milestone node tasks MAY become dispatch candidates subject to the normal scheduler safety rules

### Requirement: Milestones advance one eligible node at a time
When a milestone is automated, the system SHALL ensure that at most one milestone node task has an in-progress attempt at a time.

#### Scenario: One node attempt at a time
- **WHEN** a milestone has a node task with an in-progress attempt
- **THEN** other node tasks in the same milestone are not eligible for unattended dispatch

#### Scenario: Next eligible node is dispatchable
- **WHEN** a milestone has no in-progress attempt
- **AND** a node's predecessor nodes are all `done`
- **AND** the node task is not terminal
- **THEN** the node is eligible for dispatch (subject to automation mode and policy)

### Requirement: Milestone checkpoints act as human gates
Milestones SHALL support a checkpoint gate that requires explicit human approval before downstream work proceeds.

#### Scenario: Checkpoint blocks downstream dispatch
- **WHEN** a milestone contains a checkpoint node that is not approved
- **THEN** downstream nodes that depend on the checkpoint are not eligible for dispatch

#### Scenario: Approved checkpoint unblocks downstream nodes
- **WHEN** a human approves a checkpoint node
- **THEN** downstream nodes become eligible once their other predecessor requirements are satisfied

### Requirement: Human take-over remains explicit
The system SHALL provide a way for a human operator to pause milestone automation and take over work manually.

#### Scenario: Pause milestone automation
- **WHEN** a human pauses milestone automation
- **THEN** the scheduler SHALL stop dispatching new milestone node attempts
- **AND** existing in-progress attempts remain visible and reviewable

#### Scenario: Resume milestone automation
- **WHEN** a human resumes milestone automation
- **THEN** eligible nodes become dispatch candidates again subject to the normal scheduler rules
