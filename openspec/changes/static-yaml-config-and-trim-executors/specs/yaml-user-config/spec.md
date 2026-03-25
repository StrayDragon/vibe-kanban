## ADDED Requirements

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
The system SHALL resolve environment placeholders in YAML string values using the following precedence: `secret.env` > process/system environment.

Supported placeholders:
- `${NAME}`
- `${NAME:-default}`

#### Scenario: secret.env overrides system env
- **WHEN** `secret.env` contains `OPENAI_API_KEY=from_secret`
- **AND** the process/system environment contains `OPENAI_API_KEY=from_system`
- **AND** `config.yaml` contains `openai.api_key: "${OPENAI_API_KEY}"`
- **THEN** the resolved value is `from_secret`

#### Scenario: Default value is used when var is missing
- **WHEN** `config.yaml` contains `openai.api_key: "${OPENAI_API_KEY:-fallback}"`
- **AND** neither `secret.env` nor the process/system environment defines `OPENAI_API_KEY`
- **THEN** the resolved value is `fallback`

#### Scenario: Missing var without default fails
- **WHEN** `config.yaml` contains `openai.api_key: "${OPENAI_API_KEY}"`
- **AND** neither `secret.env` nor the process/system environment defines `OPENAI_API_KEY`
- **THEN** config validation fails with an error that references `OPENAI_API_KEY`
- **AND** the system records the error for status/diagnostics
- **AND** a reload attempt keeps the last known good configuration active

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
