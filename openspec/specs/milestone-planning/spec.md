# milestone-planning Specification

## Purpose
TBD - created by archiving change add-milestone-guide-agent-and-plan-apply. Update Purpose after archive.
## Requirements
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

### Requirement: Milestone planner UX SHALL not require manual plan JSON editing
The system SHALL provide a guided milestone planning experience that allows operators to preview and apply a milestone plan produced by the Guide agent without manually copy/pasting or editing a JSON payload.

#### Scenario: Planner offers preview/apply from latest guide output
- **WHEN** an operator opens the milestone planner surface
- **AND** the latest guide attempt output contains a detectable `milestone-plan-v1` payload
- **THEN** the UI offers a one-click plan preview action
- **AND** the UI offers an apply action only after a successful preview
- **AND** the operator does not need to interact with a raw JSON textarea to proceed

#### Scenario: Planner handles missing plan output with actionable guidance
- **WHEN** an operator opens the milestone planner surface
- **AND** no detectable `milestone-plan-v1` payload exists in the latest guide output
- **THEN** the UI shows a clear empty/error state explaining that no plan was detected
- **AND** the UI provides an operator action to re-run or continue the guide attempt to re-emit the plan block

#### Scenario: Planner surfaces invalid plan payload errors without requiring JSON debugging
- **WHEN** the latest guide output includes a plan block that fails JSON parsing or schema validation
- **THEN** the system surfaces a concise, actionable validation error to the operator
- **AND** the operator can retry by asking the guide to re-emit a valid plan payload

### Requirement: Raw plan payload exposure SHALL be gated
The system SHALL hide raw milestone plan payload editing surfaces by default, while allowing a gated debug affordance to view/copy the detected payload for troubleshooting.

#### Scenario: Default UI does not show a raw plan textarea
- **WHEN** an operator uses the milestone planner surface under normal configuration
- **THEN** the UI does not render a raw plan JSON textarea as a primary control

#### Scenario: Gated debug view reveals a copyable payload
- **WHEN** an operator enables the planner debug mode (development-only or explicitly gated internal-tools setting)
- **AND** a detectable plan payload exists
- **THEN** the UI shows the raw plan payload in a read-only view
- **AND** the operator can copy it for debugging or support

### Requirement: Plan detection SHALL return a structured result
The system SHALL provide a structured plan-detection action so clients can distinguish "not found" from "invalid" and present clear next steps.

#### Scenario: Detection returns found with provenance
- **WHEN** a client requests the latest detected plan for a guide session/attempt
- **AND** a detectable `milestone-plan-v1` payload exists
- **THEN** the response returns `status=found`
- **AND** it includes the parsed `MilestonePlanV1` payload
- **AND** it includes provenance identifying the source turn/message

#### Scenario: Detection returns not_found when no plan exists
- **WHEN** a client requests the latest detected plan for a guide session/attempt
- **AND** no detectable plan payload exists
- **THEN** the response returns `status=not_found`

#### Scenario: Detection returns invalid with a validation summary
- **WHEN** a client requests the latest detected plan for a guide session/attempt
- **AND** a candidate payload exists but is invalid JSON or fails schema validation
- **THEN** the response returns `status=invalid`
- **AND** it includes a concise validation summary suitable for operator display

