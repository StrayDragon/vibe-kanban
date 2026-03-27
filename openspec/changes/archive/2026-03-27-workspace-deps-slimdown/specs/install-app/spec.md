# install-app Specification (Delta)

## ADDED Requirements

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

