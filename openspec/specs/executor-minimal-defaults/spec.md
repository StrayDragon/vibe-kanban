# executor-minimal-defaults Specification

## Purpose
Keep the default build/CI surface minimal by supporting only core executors by default, while
allowing non-core executors to remain opt-in.
## Requirements
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

### Requirement: Default workspace verification compiles only core executors
The system SHALL keep the default workspace build and verification path minimal by compiling and testing only the core executors (Claude Code and Codex) by default.

Non-core executors MAY exist in the repository, but they MUST be opt-in (feature-gated and/or excluded from the default workspace members set).

#### Scenario: Default build uses the minimal executor set
- **WHEN** the system is built and tested with the default workspace configuration
- **THEN** only Claude Code and Codex executors are required to compile and run

### Requirement: Non-core executors remain opt-in and do not block core CI
The repository SHALL ensure that enabling or disabling non-core executor features does not block the core CI path.

#### Scenario: Core CI does not fail due to non-core executors
- **WHEN** core CI runs without non-core executor features
- **THEN** the build and tests succeed without requiring those executors to compile

