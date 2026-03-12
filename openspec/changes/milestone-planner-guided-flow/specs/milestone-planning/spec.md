## ADDED Requirements

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

