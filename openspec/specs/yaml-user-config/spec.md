# yaml-user-config Specification

## Purpose
Define how VK loads operator configuration from a canonical config directory, resolves templates and secrets safely, and applies updates via explicit reload.
## Requirements
### Requirement: OS user config directory is the canonical config root
The system SHALL resolve a single OS user config directory for VK configuration and treat it as the canonical source of truth.

#### Scenario: Default config dir resolution
- **WHEN** `VK_CONFIG_DIR` is unset
- **THEN** the system resolves the config dir using OS conventions (for example `~/.config/vk/` on Linux/macOS)

#### Scenario: Override config dir resolution
- **WHEN** `VK_CONFIG_DIR` is set
- **THEN** the system uses that directory as the config root

### Requirement: YAML config and secret overlay files are loaded from the config directory
The system SHALL load `config.yaml` from the config directory and SHALL attempt to load `secret.env` (dotenv format) from the same directory.

#### Scenario: Missing secret.env is allowed
- **WHEN** `secret.env` is absent
- **THEN** config loading proceeds using only the process/system environment

#### Scenario: Missing config.yaml falls back to defaults
- **WHEN** `config.yaml` is absent
- **THEN** the system uses the default configuration

### Requirement: Template resolution is deterministic and uses secret.env precedence
The system SHALL resolve environment placeholders in whitelisted YAML string fields using the following precedence: `secret.env` > process/system environment.

Supported placeholders:
- `{{env.NAME}}`
- `{{env.NAME:-default}}`
- `{{secret.NAME}}`

Templates MUST NOT be allowed in arbitrary string fields. If a non-whitelisted field contains template syntax, config validation SHALL fail with an error that references the field path.

#### Scenario: secret.env overrides system env
- **WHEN** `secret.env` contains `OPENAI_API_KEY=from_secret`
- **AND** the process/system environment contains `OPENAI_API_KEY=from_system`
- **AND** a whitelisted field contains `{{env.OPENAI_API_KEY}}`
- **THEN** the resolved value is `from_secret`

#### Scenario: Default value is used when var is missing
- **WHEN** a whitelisted field contains `{{env.OPENAI_API_KEY:-fallback}}`
- **AND** neither `secret.env` nor the process/system environment defines `OPENAI_API_KEY`
- **THEN** the resolved value is `fallback`

#### Scenario: Missing var without default fails
- **WHEN** a whitelisted field contains `{{env.OPENAI_API_KEY}}`
- **AND** neither `secret.env` nor the process/system environment defines `OPENAI_API_KEY`
- **THEN** config validation fails with an error that references `OPENAI_API_KEY`
- **AND** the system records the error for status/diagnostics
- **AND** a reload attempt keeps the last known good configuration active

#### Scenario: Template in a non-whitelisted field is rejected
- **WHEN** a non-whitelisted YAML string field contains `{{secret.SOME_KEY}}`
- **THEN** config validation fails with an error that references that field path

### Requirement: Config reload preserves last-known-good configuration
The system SHALL support reloading configuration at runtime and SHALL keep the last known good configuration active when a reload fails.

#### Scenario: Reload success swaps the active config snapshot
- **WHEN** a reload is triggered and the updated config is valid
- **THEN** the new configuration becomes active for subsequent operations

#### Scenario: Reload failure keeps the previous snapshot
- **WHEN** a reload is triggered and the updated config is invalid
- **THEN** the system keeps the previously loaded configuration active
- **AND** it records diagnostics describing the reload failure

### Requirement: Config load status is observable without leaking secrets
The system SHALL expose config load metadata (config dir, loaded-at timestamp, and last error summary) without exposing secret values.

#### Scenario: Status does not include secret.env values
- **WHEN** a client requests config status
- **THEN** the response does not include any values from `secret.env`

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

