# filesystem-api-boundary Specification

## Purpose
TBD - created by archiving change update-filesystem-api-boundary. Update Purpose after archive.
## Requirements
### Requirement: Workspace-scoped filesystem listing
The system SHALL restrict filesystem listing APIs to configured workspace roots.

#### Scenario: List path inside workspace
- **WHEN** a client requests a directory under a configured workspace root
- **THEN** the system returns directory entries

#### Scenario: Reject path outside workspace
- **WHEN** a client requests a path outside all configured workspace roots
- **THEN** the system rejects the request with `403`

### Requirement: Canonical path containment
The system SHALL resolve requested paths to canonical absolute paths before access checks.

#### Scenario: Reject traversal attempt
- **WHEN** a client submits a path containing traversal segments that resolve outside workspace roots
- **THEN** the request is rejected with `403`

### Requirement: Bounded repository discovery
The system SHALL run git repository discovery only within configured workspace roots.

#### Scenario: Discover repos in workspace
- **WHEN** a client requests repository discovery for a workspace root
- **THEN** only repositories under that root are returned

### Requirement: Workspace-scoped repository registration
The system SHALL restrict repository registration APIs to configured workspace roots.

#### Scenario: Register repo inside workspace roots
- **WHEN** a client registers a repository whose canonical path is under an allowed workspace root
- **THEN** the repository is registered successfully

#### Scenario: Reject register repo outside workspace roots
- **WHEN** a client registers a repository whose canonical path is outside all allowed workspace roots
- **THEN** the system rejects the request with `403`

### Requirement: Workspace-scoped repository initialization
The system SHALL restrict repository initialization APIs to configured workspace roots.

#### Scenario: Init repo inside workspace roots
- **WHEN** a client initializes a repository under an allowed workspace root
- **THEN** the repository is initialized and registered successfully

#### Scenario: Reject init repo outside workspace roots
- **WHEN** a client attempts to initialize a repository under a parent path outside all allowed workspace roots
- **THEN** the system rejects the request with `403`

