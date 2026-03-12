# project-settings-summary Specification

## Purpose
TBD - created by archiving change consolidate-project-readonly-details-into-settings. Update Purpose after archive.
## Requirements
### Requirement: Selected project settings SHALL expose essential readonly metadata
The system SHALL show essential readonly project metadata within the selected-project settings experience so operators do not need a separate project-detail page for identification and audit context.

#### Scenario: selected project metadata is visible in settings
- **WHEN** an operator opens `Settings > Projects` with a project selected
- **THEN** the UI shows the project's identifier and created / last-modified timestamps in a readonly presentation on that page

#### Scenario: metadata stays adjacent to editable settings
- **WHEN** the selected project settings view is rendered
- **THEN** the readonly metadata appears within the same project settings workflow as the editable project configuration
- **AND** the operator does not need to navigate to a separate detail-only page to inspect it

### Requirement: Lifecycle-hook settings SHALL show the latest recorded hook outcome
The system SHALL show the latest recorded lifecycle-hook outcome inside the project lifecycle-hook settings section when a selected project has hook configuration and a recorded result.

#### Scenario: latest hook outcome appears with configured hooks
- **WHEN** a selected project has lifecycle hooks configured and VK has recorded a latest hook result
- **THEN** the lifecycle-hook settings section shows the latest source task context and a compact summary of the recorded hook outcome

#### Scenario: lifecycle-hook section handles missing results cleanly
- **WHEN** a selected project has lifecycle hooks configured but no recorded hook result yet
- **THEN** the lifecycle-hook settings section shows a concise empty state instead of leaving a blank area

#### Scenario: lifecycle-hook section handles disabled hooks cleanly
- **WHEN** a selected project has no lifecycle hooks configured
- **THEN** the lifecycle-hook settings section shows a concise not-configured state
- **AND** it does not render a misleading latest-result summary

### Requirement: Project settings SHALL be the canonical human-facing summary surface
The system SHALL keep project-level readonly summary information and editable project controls in the same selected-project settings flow.

#### Scenario: operators review metadata and settings from one surface
- **WHEN** an operator needs to inspect project metadata, lifecycle-hook diagnostics, and editable project settings
- **THEN** the selected-project settings flow provides those concerns together in one human-facing surface

#### Scenario: readonly summary does not duplicate existing editable controls unnecessarily
- **WHEN** the selected-project settings view includes a readonly summary
- **THEN** it avoids duplicating existing editable scheduler controls as separate readonly blocks unless they add distinct diagnostic value

