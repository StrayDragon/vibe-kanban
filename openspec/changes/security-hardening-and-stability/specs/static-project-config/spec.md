# static-project-config Specification

## MODIFIED Requirements

### Requirement: Project and repo configuration is file-based
The system SHALL load project and repository definitions from file-based YAML configuration under the OS config directory and SHALL NOT require DB-backed settings for these definitions.

The canonical sources are:
- `projects.yaml`
- `projects.d/*.yaml|yml` (merged deterministically)

Inline `projects` within `config.yaml` SHALL be used only when no `projects.yaml` or `projects.d/*` files exist.

#### Scenario: Project list comes from projects.yaml
- **WHEN** `projects.yaml` (or `projects.d/*.yaml|yml`) defines one or more projects
- **THEN** the system lists those projects as the selectable set for task creation and policy evaluation

#### Scenario: Inline projects are used only as a fallback
- **WHEN** neither `projects.yaml` nor any `projects.d/*.yaml|yml` files exist
- **THEN** the system MAY fall back to inline `projects` defined in `config.yaml`

## ADDED Requirements

### Requirement: Project repo APIs do not expose setup/cleanup script bodies
The system SHALL NOT expose repository `setup_script`, `cleanup_script`, or other executable script bodies in project/repository API responses.

The system MAY expose safe metadata (for example, whether a script is configured) to support UI display.

#### Scenario: Repository detail response is script-free
- **WHEN** a client requests a project repository detail endpoint
- **THEN** the response does not include the script body contents for setup/cleanup scripts

