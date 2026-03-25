## MODIFIED Requirements

### Requirement: Selected project settings SHALL expose essential readonly metadata
The system SHALL show essential readonly project metadata within the selected-project settings experience so operators do not need a separate project-detail page for identification and audit context.

#### Scenario: selected project metadata is visible in settings
- **WHEN** an operator opens `Settings > Projects` with a project selected
- **THEN** the UI shows the project's identifier and repository path(s) in a readonly presentation on that page
- **AND** if the project is configured from YAML, the UI shows the config source (`config.yaml`) as readonly metadata

#### Scenario: metadata stays adjacent to editable settings
- **WHEN** the selected project settings view is rendered
- **THEN** the readonly metadata appears within the same project settings workflow as the editable project configuration
- **AND** the operator does not need to navigate to a separate detail-only page to inspect it
