# project-settings-summary Specification (Delta)

## MODIFIED Requirements

### Requirement: Selected project settings SHALL expose essential readonly metadata
The system SHALL show essential readonly project metadata within the settings experience so operators do not need a separate project-detail page for identification and audit context.

#### Scenario: project metadata is visible in settings
- **WHEN** an operator opens the Settings Projects section
- **THEN** the UI shows the project's identifier and repository path(s) in a readonly presentation
- **AND** if the project is configured from YAML, the UI shows the config source (`projects.yaml` / `projects.d/*.yaml`) as readonly metadata

#### Scenario: metadata stays adjacent to configuration guidance
- **WHEN** the settings view is rendered
- **THEN** the readonly metadata appears within the same settings workflow as the configuration guidance (copy snippets, file paths, reload help)
- **AND** the operator does not need to navigate to a separate detail-only page to inspect it

