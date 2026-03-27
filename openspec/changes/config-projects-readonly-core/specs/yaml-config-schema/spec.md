# yaml-config-schema Specification (Delta)

## ADDED Requirements

### Requirement: CLI can upsert config schemas
The system SHALL provide a CLI command that generates (or updates) `config.schema.json` and `projects.schema.json` under the config directory.

#### Scenario: CLI upserts schemas successfully
- **WHEN** an operator runs the schema upsert command
- **THEN** `config.schema.json` exists in the config directory
- **AND** `projects.schema.json` exists in the config directory

### Requirement: Server startup does not require schema write side effects
The system SHALL NOT require writing schema files as part of the server startup path.

#### Scenario: Read-only config directory does not block server startup
- **WHEN** the config directory is not writable
- **THEN** the server can still start and serve core APIs
- **AND** schema generation can be performed separately via CLI when desired

