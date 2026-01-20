# cli-dependency-preflight Specification

## Purpose
TBD - created by archiving change add-stability-hardening-ops. Update Purpose after archive.
## Requirements
### Requirement: CLI dependency preflight
The system SHALL provide a preflight check that reports availability and authentication status for the selected coding agent CLI and the GitHub CLI.

#### Scenario: Agent CLI unavailable
- **WHEN** a client requests CLI preflight for a coding agent whose executable is not available
- **THEN** the response marks the agent dependency as unavailable

#### Scenario: GitHub CLI unauthenticated
- **WHEN** a client requests CLI preflight and `gh` is installed but unauthenticated
- **THEN** the response marks the GitHub CLI dependency as unauthenticated

#### Scenario: All dependencies ready
- **WHEN** a client requests CLI preflight and required CLIs are installed and authenticated
- **THEN** the response marks both dependencies as ready

