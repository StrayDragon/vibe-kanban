# filesystem-api-boundary Specification

## ADDED Requirements

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

