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

