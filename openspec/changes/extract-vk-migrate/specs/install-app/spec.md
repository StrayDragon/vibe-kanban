# install-app Specification (Delta)

## ADDED Requirements

### Requirement: Migration CLI is available as a standalone binary
The system SHALL provide a standalone operator CLI binary (named `vk`) that can be built and run without npm tooling and without starting the server.

#### Scenario: Build vk alongside the server
- **WHEN** the workspace is built from source using Cargo
- **THEN** the `vk` binary can be produced and runs with `--help`

#### Scenario: Migrations can be run without a running server
- **WHEN** an operator runs `vk migrate ...` to export legacy configuration
- **THEN** the tool produces migrated output files without requiring the server process to be running

### Requirement: Migration output is non-destructive by default
The migration CLI SHALL default to writing new output files and SHALL NOT overwrite existing user configuration files unless explicitly requested.

#### Scenario: Output-only migration writes timestamped files
- **WHEN** an operator runs a migration command that produces configuration output
- **THEN** the tool writes new files with a distinct `*.migrated.<timestamp>.*` naming scheme
- **AND** existing `config.yaml`, `projects.yaml`, and `secret.env` remain unchanged

### Requirement: Secret outputs use restrictive file permissions
When the migration CLI writes secret files (for example `secret.env`), it SHALL create them with restrictive permissions and SHALL NOT leave secret-bearing temporary or backup files with broader permissions.

#### Scenario: Migrated secret.env output is mode 0600
- **WHEN** the migration CLI writes a migrated secret output file
- **THEN** the file mode is `0600` (owner read/write only)
