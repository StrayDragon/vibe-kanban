# yaml-user-config Specification (Delta)

## ADDED Requirements

### Requirement: Config changes are applied only by explicit reload
The system SHALL NOT automatically apply on-disk YAML configuration changes. Configuration changes SHALL become active only when an explicit reload is triggered.

#### Scenario: File edits do not change active config until reload
- **WHEN** an operator edits `config.yaml` or `projects.yaml` on disk
- **THEN** the active configuration used for subsequent operations remains the previously loaded snapshot
- **AND** the system marks the configuration as dirty for observability

#### Scenario: Reload applies changes and clears dirty state
- **WHEN** an operator triggers a reload and the updated configuration is valid
- **THEN** the new configuration becomes active for subsequent operations
- **AND** the dirty indicator is cleared

### Requirement: Dirty state is observable
The system SHALL expose whether the on-disk configuration has changed since the last successful load (dirty state) without exposing any secret values.

#### Scenario: Status includes dirty without leaking secrets
- **WHEN** a client requests config status
- **THEN** the response includes a dirty indicator
- **AND** the response does not include any values from `secret.env`

### Requirement: Public config view is consistent and does not resolve templates
The system SHALL provide a public/read-only configuration view for UI/API display that does not resolve `{{secret.*}}`/`{{env.*}}` templates and is consistent with the last successfully loaded runtime configuration snapshot.

#### Scenario: Public view preserves placeholders
- **WHEN** a YAML field contains `{{secret.NAME}}` or `{{env.NAME}}`
- **THEN** the public config view contains the literal placeholder text
- **AND** the runtime config may contain the resolved value for execution purposes

