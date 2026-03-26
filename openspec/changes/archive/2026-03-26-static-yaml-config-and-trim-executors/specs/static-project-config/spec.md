## ADDED Requirements

### Requirement: Project and repo configuration is file-based
The system SHALL load project and repository definitions from the YAML configuration and SHALL NOT require DB-backed settings for these definitions.

#### Scenario: Project list comes from YAML
- **WHEN** `config.yaml` defines one or more projects
- **THEN** the system lists those projects as the selectable set for task creation and policy evaluation

### Requirement: Projects have stable identifiers
Each configured project SHALL have a stable identifier that is used to associate runtime data (tasks/attempts/workspaces) with that project.

The identifier:
- SHALL be a UUID string.
- SHALL be explicitly present in `config.yaml` (not auto-generated at runtime).
- SHALL be unique across all configured projects.

#### Scenario: Configured project id is used for new tasks
- **WHEN** an operator creates a new task under a configured project
- **THEN** the created runtime records reference that project’s configured identifier

#### Scenario: Missing project id is rejected
- **WHEN** a project entry in `config.yaml` omits its identifier
- **THEN** config validation fails with an error that references the missing project id

#### Scenario: Duplicate project ids are rejected
- **WHEN** two project entries in `config.yaml` share the same identifier
- **THEN** config validation fails with an error that references the duplicate identifier

### Requirement: Orphaned runtime history is handled gracefully
The system SHALL handle runtime records that reference a project identifier that is missing from the current YAML configuration.

#### Scenario: Task references missing project
- **WHEN** a runtime task references a project identifier that is not present in `config.yaml`
- **THEN** the UI/API surfaces the task under an “Unknown project” placeholder without crashing

### Requirement: Optional export of existing DB-backed project/repo settings
The system SHALL provide a one-shot export that writes a YAML representation of existing DB-backed project/repo settings into the config directory.

#### Scenario: Export writes YAML without secrets
- **WHEN** an operator runs the export tool
- **THEN** the exported YAML is written under the config directory and does not include secret token values
