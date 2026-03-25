## ADDED Requirements

### Requirement: JSON Schema is generated for the YAML config
The system SHALL generate a JSON Schema describing the YAML configuration and write it as `config.schema.json` in the config directory.

#### Scenario: Schema is generated when requested
- **WHEN** schema generation is triggered
- **THEN** `config.schema.json` exists in the config directory and matches the current config schema

### Requirement: Schema generation is safe and does not include secret values
The system SHALL NOT embed any secret values into the generated schema and SHALL represent secret fields as plain strings.

#### Scenario: Schema does not leak runtime secrets
- **WHEN** a configuration includes secret-bearing fields (for example API keys)
- **THEN** the generated schema contains only types/constraints and no runtime values

### Requirement: Schema can be associated with config.yaml for YAML LSP
The system SHALL document a supported way to associate `config.yaml` with the generated `config.schema.json` for YAML LSP validation and completion.

#### Scenario: yaml-language-server directive references the generated schema
- **WHEN** `config.yaml` contains `# yaml-language-server: $schema=./config.schema.json`
- **THEN** YAML LSP tooling can use the generated schema for validation and completion
