# codex-protocol-compat-gate Specification

## Purpose
TBD - created by archiving change codex-dynamic-tools. Update Purpose after archive.
## Requirements

### Requirement: Codex protocol compatibility status
The system SHALL determine a protocol compatibility status for the local Codex executor based on the locally installed `codex` binary and VK’s expected Codex app-server protocol surface.

#### Scenario: Codex is not installed
- **WHEN** the system checks Codex compatibility and the `codex` executable cannot be resolved
- **THEN** the system reports the compatibility status as `not_installed`

#### Scenario: Compatibility check fails
- **WHEN** the system checks Codex compatibility and the compatibility check errors (non-zero exit, missing permissions, malformed output)
- **THEN** the system reports the compatibility status as `unknown`
- **AND** includes an actionable diagnostic message suitable for display in settings

#### Scenario: Protocol is compatible
- **WHEN** the system checks Codex compatibility and the runtime protocol fingerprint matches the expected fingerprint
- **THEN** the system reports the compatibility status as `compatible`

#### Scenario: Protocol is incompatible
- **WHEN** the system checks Codex compatibility and the runtime protocol fingerprint does not match the expected fingerprint
- **THEN** the system reports the compatibility status as `incompatible`

### Requirement: Codex executor is disabled when incompatible
The system SHALL disable the Codex executor when compatibility status is `incompatible`.

#### Scenario: User attempts to start a Codex run while incompatible
- **WHEN** a user attempts to start a Codex-backed attempt and the compatibility status is `incompatible`
- **THEN** the system rejects the spawn request
- **AND** returns an actionable error message that instructs the user to upgrade VK or align `codex-cli`

#### Scenario: Settings UI reflects incompatibility
- **WHEN** the compatibility status is `incompatible`
- **THEN** the settings UI shows Codex as unavailable/disabled
- **AND** displays remediation steps

### Requirement: Compatibility check caching and refresh
The system SHALL cache Codex compatibility results and revalidate them when the resolved Codex command identity changes.

#### Scenario: Cached result used
- **WHEN** the compatibility status is requested repeatedly without changes to the resolved Codex command identity
- **THEN** the system returns the cached compatibility status without re-running the expensive compatibility check

#### Scenario: Revalidate on Codex command change
- **WHEN** the resolved Codex command identity (path and/or version) changes
- **THEN** the system re-runs the compatibility check and updates the reported status

#### Scenario: Manual refresh
- **WHEN** the user requests a manual refresh of Codex compatibility in settings
- **THEN** the system re-runs the compatibility check and updates the reported status
