# install-app Specification

## Purpose
TBD - created by archiving change remove-npx-distribution. Update Purpose after archive.
## Requirements
### Requirement: Source Build Installation
The system SHALL support running from locally built Rust binaries without npm/npx distribution.

#### Scenario: Build and run server with embedded frontend
- **WHEN** the frontend is built into `frontend/dist` before compiling the server
- **THEN** the resulting `server` binary serves the frontend UI without a separate frontend process

#### Scenario: Build the MCP server as a standalone binary
- **WHEN** the `mcp_task_server` binary is built alongside the server
- **THEN** users can run the MCP server without npm tooling

### Requirement: Vibe Kanban Installation Uses Direct Binaries
The system SHALL NOT require npm/npx to install or run the Vibe Kanban server or Vibe Kanban MCP server binaries.

#### Scenario: Official instructions omit npx usage for Vibe Kanban
- **WHEN** users follow the documented Vibe Kanban installation and MCP setup steps
- **THEN** they do not see `npx vibe-kanban` or `vibe-kanban@latest` commands

#### Scenario: Default MCP configuration uses the MCP binary
- **WHEN** a default MCP server entry for Vibe Kanban is provided
- **THEN** it uses `mcp_task_server` as the command instead of an npx wrapper

### Requirement: No NPM Distribution Pipeline
The system SHALL NOT include an npm-based distribution workflow for Vibe Kanban binaries.

#### Scenario: CI release workflows do not publish npm packages
- **WHEN** release workflows are run
- **THEN** no steps build or publish an npm package for Vibe Kanban

### Requirement: Release Artifacts Are Linux x86_64 Only
The system SHALL publish release artifacts for Linux x86_64 builds only.

#### Scenario: CI release workflow builds a single Linux target
- **WHEN** the release workflow runs
- **THEN** it produces binaries labeled for `linux-x64` and no other platforms

### Requirement: Generated environment variable reference
The system SHALL provide a generated environment-variable reference document and SHALL validate it in CI to prevent documentation drift.

#### Scenario: Generated env doc exists
- **WHEN** the repository is checked out
- **THEN** `docs/env.gen.md` exists and documents supported environment-variable knobs

#### Scenario: CI validates the generated env doc
- **WHEN** CI runs
- **THEN** the `generate-env-docs:check` step fails if `docs/env.gen.md` is out of date

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

### Requirement: Server-only build is supported
The system SHALL support a server-only build/run mode where the backend provides core HTTP APIs and MCP without embedding or serving the frontend UI.

This mode is intended for multi-node deployments where a single frontend is served separately and multiple backend servers form a cluster.

#### Scenario: Server-only node serves APIs without a frontend bundle
- **WHEN** the server is built in server-only mode (without embedded frontend assets)
- **THEN** core `/api/**` endpoints are available
- **AND** the UI asset routes do not require an embedded `frontend/dist` bundle to exist

### Requirement: Default build still supports embedded frontend
The default build/distribution SHALL continue to support serving the embedded frontend UI from the server binary.

#### Scenario: Default build serves embedded UI
- **WHEN** the frontend is built into `frontend/dist` before compiling the server (default build)
- **THEN** the resulting `server` binary serves the frontend UI without a separate frontend process

