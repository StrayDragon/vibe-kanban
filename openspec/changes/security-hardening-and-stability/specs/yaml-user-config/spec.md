# yaml-user-config Specification

## ADDED Requirements

### Requirement: Public config view is safe for API responses
The system SHALL provide a "public" configuration view for API/UI consumption that does not resolve `{{secret.*}}` / `{{env.*}}` templates and does not include secret values.

The system SHALL use this public view (or an equivalent redacted DTO) for any API responses that surface configuration-derived data.

#### Scenario: Public config does not resolve templates
- **WHEN** `config.yaml` contains a string field with `{{secret.NAME}}` or `{{env.NAME}}`
- **THEN** the public config view preserves the placeholder text (or omits the field)
- **AND** it does not contain the expanded secret value

#### Scenario: Config-derived APIs use the public view
- **WHEN** an API response includes configuration-derived fields (for example, project/repo settings)
- **THEN** the response does not include expanded `{{secret.*}}` values

### Requirement: Reload swaps config snapshots atomically
The system SHALL apply a config reload as an atomic snapshot swap across all config-derived runtime state (runtime config, public config, and config status/diagnostics).

#### Scenario: No mixed snapshot is observable
- **WHEN** a reload succeeds
- **THEN** subsequent operations observe a single consistent configuration generation
- **AND** readers do not observe a mixture of old and new config-derived state

### Requirement: Reload requests are serialized
The system SHALL serialize reload triggers (file watcher and explicit reload API) so that only one reload is executing at a time.

#### Scenario: Concurrent reload triggers do not race
- **WHEN** multiple reload triggers occur concurrently
- **THEN** reload executions are processed sequentially
- **AND** the final active snapshot corresponds to the last successfully loaded configuration

