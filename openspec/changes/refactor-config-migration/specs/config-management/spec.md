## ADDED Requirements
### Requirement: Single-schema config deserialization
The system SHALL deserialize configuration using the latest schema with defaults for missing fields and without version-specific migrations.

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

#### Scenario: Invalid JSON
- **WHEN** the config file contains malformed JSON
- **THEN** the loader returns the default Config and logs a warning

### Requirement: Persist latest schema on save
The system SHALL persist configuration using the latest schema and config_version.

#### Scenario: Save after load
- **WHEN** a Config is saved
- **THEN** the serialized config_version equals the latest value and uses the latest field set
