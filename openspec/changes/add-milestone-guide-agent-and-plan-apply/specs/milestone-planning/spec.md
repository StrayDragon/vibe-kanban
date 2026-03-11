## ADDED Requirements

### Requirement: Versioned milestone plan schema
The system SHALL define and accept a versioned milestone planning payload (“Milestone Plan”) that can be validated, previewed, and applied to a milestone.

#### Scenario: Supported plan version is accepted
- **WHEN** a caller submits a milestone plan with a supported `schema_version`
- **THEN** the system validates the plan and proceeds to preview or apply

#### Scenario: Unsupported plan version is rejected
- **WHEN** a caller submits a milestone plan with an unsupported `schema_version`
- **THEN** the system rejects the request with a structured validation error

### Requirement: Plan preview is pure and returns a deterministic diff
The system SHALL provide a plan preview action that validates a plan and returns a deterministic summary of what would change, without mutating any persisted state.

#### Scenario: Preview returns an actionable diff summary
- **WHEN** a caller previews a valid milestone plan
- **THEN** the response includes a summary of milestone metadata changes
- **AND** the response includes which node tasks would be created vs linked
- **AND** the response includes the resulting node/edge graph shape

### Requirement: Plan apply is explicit and atomic
The system SHALL apply milestone plans only through an explicit apply action. Plan application SHALL be atomic: it either fully succeeds or produces no persisted changes.

#### Scenario: Apply creates tasks and updates the milestone graph
- **WHEN** a caller applies a valid milestone plan
- **THEN** the system creates any missing tasks required by the plan
- **AND** the system updates milestone metadata and graph in a single atomic operation
- **AND** subsequent reads return the updated milestone graph and metadata

#### Scenario: Invalid apply does not partially mutate state
- **WHEN** a caller applies a milestone plan that fails validation
- **THEN** the system rejects the request
- **AND** no partial tasks, nodes, or edges are persisted

### Requirement: Plan apply is idempotent under retries
The system SHALL support retry-safe plan application.

#### Scenario: Retried apply returns a stable result
- **WHEN** a client retries the same apply request using the system's idempotency mechanism
- **THEN** the system returns the same logical result without duplicating tasks or edges

### Requirement: Planner provenance is persisted and inspectable
The system SHALL persist enough provenance for humans and tools to understand where a planned milestone structure came from.

#### Scenario: Plan-created tasks are attributable
- **WHEN** the system creates tasks as part of a milestone plan application
- **THEN** those tasks record a distinct created-by attribution suitable for UI display and audit

#### Scenario: Applied plan is discoverable
- **WHEN** a milestone plan is applied
- **THEN** operators can inspect that a plan was applied and when it occurred without scraping raw logs

### Requirement: Plans can be surfaced from agent output in a stable format
The system SHALL define a stable, machine-detectable encoding for milestone plans in agent output so interactive clients can offer preview/apply without ambiguous parsing.

#### Scenario: UI detects a plan block in agent output
- **WHEN** an agent emits a milestone plan using the canonical encoding
- **THEN** the UI can extract the plan payload and offer preview/apply actions

