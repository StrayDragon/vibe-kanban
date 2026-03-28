# cli-dependency-preflight Specification (Delta)

## MODIFIED Requirements

### Requirement: CLI dependency preflight
The system SHALL provide a preflight check that reports availability and authentication status for the selected coding agent CLI and the GitHub CLI, and SHALL report availability for the Git CLI (`git`).

#### Scenario: Agent CLI unavailable
- **WHEN** a client requests CLI preflight for a coding agent whose executable is not available
- **THEN** the response marks the agent dependency as unavailable

#### Scenario: Git CLI unavailable
- **WHEN** a client requests CLI preflight and the `git` executable is not available or not runnable
- **THEN** the response marks the Git dependency as unavailable

#### Scenario: GitHub CLI unauthenticated
- **WHEN** a client requests CLI preflight and `gh` is installed but unauthenticated
- **THEN** the response marks the GitHub CLI dependency as unauthenticated

#### Scenario: All dependencies ready
- **WHEN** a client requests CLI preflight and required CLIs are installed and authenticated
- **THEN** the response marks the agent dependency, Git dependency, and GitHub CLI dependency as ready

