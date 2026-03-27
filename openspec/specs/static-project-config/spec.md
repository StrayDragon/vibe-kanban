# static-project-config Specification

## Purpose
Define how projects and repositories are configured from file-first YAML sources and how runtime data associates to stable project identifiers without requiring DB-backed settings.
## Requirements
### Requirement: Project and repo configuration is file-based
The system SHALL load project and repository definitions from the YAML configuration and SHALL NOT require DB-backed settings for these definitions.

Canonical sources:
- `projects.yaml` in the config directory
- optional `projects.d/*.yaml|yml` files in the same directory (merged deterministically)

#### Scenario: Project list comes from projects.yaml
- **WHEN** `projects.yaml` defines one or more projects
- **THEN** the system lists those projects as the selectable set for task creation and policy evaluation

#### Scenario: DB does not act as a configuration source
- **WHEN** the database contains project records
- **AND** `projects.yaml` / `projects.d` define no projects
- **THEN** the system lists no configured projects

### Requirement: Projects have stable identifiers
Each configured project SHALL have a stable identifier that is used to associate runtime data (tasks/attempts/workspaces) with that project.

The identifier:
- SHALL be a UUID string.
- SHALL be explicitly present in `projects.yaml` / `projects.d` (not auto-generated at runtime).
- SHALL be unique across all configured projects.

#### Scenario: Configured project id is used for new tasks
- **WHEN** an operator creates a new task under a configured project
- **THEN** the created runtime records reference that project’s configured identifier

#### Scenario: Missing project id is rejected
- **WHEN** a project entry omits its identifier
- **THEN** config validation fails with an error that references the missing project id

#### Scenario: Duplicate project ids are rejected
- **WHEN** two project entries share the same identifier
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

### Requirement: DB project records are derived from YAML when needed
If runtime persistence requires a database project record (for example for foreign keys or historical metadata), the system SHALL derive that record from the configured YAML project entry and MAY create it on demand.

#### Scenario: Creating a task ensures the DB project record exists
- **WHEN** a client creates a task under a configured project
- **THEN** the database contains a project record for that project id
- **AND** the configuration source of truth remains the YAML files

