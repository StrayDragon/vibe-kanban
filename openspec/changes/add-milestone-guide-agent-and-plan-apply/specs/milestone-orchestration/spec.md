## ADDED Requirements

### Requirement: Milestones support guided planning inputs without changing dispatch semantics
Milestones SHALL support applying a validated planning result that updates milestone metadata and graph structure, while keeping milestone dispatch semantics unchanged.

#### Scenario: Plan application does not directly start attempts
- **WHEN** a user applies a validated milestone plan
- **THEN** the system updates the milestone structure
- **AND** the system SHALL NOT bypass the scheduler by directly starting node attempts as part of the apply action

#### Scenario: Automation mode is preserved unless explicitly changed
- **WHEN** a milestone plan application does not request an automation mode change
- **THEN** the milestone's current automation mode remains unchanged

### Requirement: Milestone baseline ref updates remain safe
Milestones SHALL allow planning workflows to update the milestone baseline reference used by milestone nodes.

#### Scenario: Baseline ref update is validated
- **WHEN** a plan application requests a milestone baseline ref update
- **THEN** the system validates the new baseline ref is non-empty and syntactically valid

