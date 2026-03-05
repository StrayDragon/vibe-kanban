# editor-integration Specification

## Purpose
TBD - created by archiving change settings-editor-none-and-config-cleanup. Update Purpose after archive.
## Requirements
### Requirement: Editor integration can be disabled
The system SHALL support a user-selected editor configuration value that disables editor integration.

#### Scenario: User selects disabled editor
- **WHEN** the user’s editor type is set to `NONE`
- **THEN** the system treats editor integration as disabled

### Requirement: UI hides “Open in …” affordances when disabled
When editor integration is disabled, the UI SHALL hide all affordances that would open files or projects in an external editor (including any “Open in …” prompts, buttons, or icons).

#### Scenario: Navbar IDE icon is hidden
- **WHEN** editor integration is disabled
- **THEN** the navbar does not render an IDE open button/icon

#### Scenario: Task attempt actions do not offer open-in-editor
- **WHEN** editor integration is disabled
- **THEN** task attempt actions do not include an “Open in …” option

### Requirement: Editor availability indicators are not shown when disabled
When editor integration is disabled, the UI SHALL NOT show editor availability checks or “not found in PATH” warnings.

#### Scenario: Availability indicator is suppressed
- **WHEN** editor integration is disabled
- **THEN** the UI does not render editor availability status

### Requirement: Open-editor endpoints reject requests when disabled
The API SHALL reject “open editor” requests when editor integration is disabled.

#### Scenario: Open-editor endpoints reject requests when disabled
- **WHEN** a client calls `POST /api/task-attempts/{attempt_id}/open-editor` while editor integration is disabled
- **THEN** the server returns a `400 Bad Request` (or equivalent validation error) indicating editor integration is disabled

- **WHEN** a client calls `POST /api/projects/{project_id}/open-editor` while editor integration is disabled
- **THEN** the server returns a `400 Bad Request` (or equivalent validation error) indicating editor integration is disabled
