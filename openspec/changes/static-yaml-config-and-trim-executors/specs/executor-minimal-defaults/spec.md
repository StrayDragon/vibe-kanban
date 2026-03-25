## ADDED Requirements

### Requirement: Default distribution supports only Claude Code and Codex
The system SHALL support only the Claude Code and Codex executors in the default build/distribution.

#### Scenario: Default executor set is minimal
- **WHEN** a client requests the available executor/profile list in a default build
- **THEN** only Claude Code and Codex executors are offered

### Requirement: Non-core executors are opt-in
The system SHALL require explicit build-time enablement for any executor other than Claude Code and Codex.

#### Scenario: Feature-gated executor is unavailable by default
- **WHEN** the system is built without a non-core executor feature
- **THEN** that executor is not reported as available and cannot be selected

### Requirement: Config rejects unavailable executors with clear diagnostics
The system SHALL reject configurations that reference an executor that is not available in the running build and SHALL report a clear error.

#### Scenario: Unavailable executor in config is rejected
- **WHEN** a profile or project policy references an unavailable executor id
- **THEN** config validation fails with an error that references the executor id and the supported executor set
