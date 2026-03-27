# executor-minimal-defaults Specification (Delta)

## ADDED Requirements

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

