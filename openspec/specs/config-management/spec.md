## MODIFIED Requirements

### Requirement: Single-schema config deserialization
The system SHALL deserialize configuration from the canonical YAML config using the latest schema with defaults for missing fields and without version-specific migrations.

#### Scenario: Missing optional fields
- **WHEN** a config file omits optional fields
- **THEN** the loader returns a Config with defaults applied

#### Scenario: Unknown fields present
- **WHEN** a config file contains unknown fields
- **THEN** the loader ignores them and still returns a Config

#### Scenario: Non-latest version tag
- **WHEN** config_version is missing or not equal to the latest value
- **THEN** the loader still parses using the latest schema and defaults

### Requirement: Config fallback on read failure
The system SHALL return default configuration when the config file is missing or invalid.

#### Scenario: Missing config file
- **WHEN** the config file cannot be read
- **THEN** the loader returns the default Config

#### Scenario: Invalid YAML
- **WHEN** the config file contains malformed YAML
- **THEN** the loader returns the default Config and logs a warning

### Requirement: Config endpoints remain stable
The system SHALL preserve existing config endpoints and response shapes while server-side modules are reorganized.

#### Scenario: Settings UI continues to load
- **WHEN** the frontend requests `/api/info`
- **THEN** the response still contains the expected `config` payload and required metadata

## REMOVED Requirements

### Requirement: Persist latest schema on save
**Reason**: VK configuration is file-first YAML and is not persisted/rewritten by VK as part of normal runtime operation.
**Migration**: Operators edit `config.yaml` directly (with YAML LSP using `config.schema.json`) and trigger a reload.
