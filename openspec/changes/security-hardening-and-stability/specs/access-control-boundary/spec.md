# access-control-boundary Specification

## ADDED Requirements

### Requirement: Token mode requires a non-empty token
When `accessControl.mode` is `token`, the configured `accessControl.token` MUST be a non-empty string.

If `accessControl.mode=token` but the token is missing or empty, the system SHALL treat access control as misconfigured and SHALL reject `/api/**` requests (HTTP/SSE/WebSocket) with a standard `ApiResponse` error payload.

#### Scenario: Missing token rejects HTTP API requests
- **WHEN** `accessControl.mode` is `token`
- **AND** `accessControl.token` is missing or empty
- **AND** a client requests any `/api/**` HTTP endpoint
- **THEN** the system returns a non-2xx status code
- **AND** the response body is a standard `ApiResponse` error payload

#### Scenario: Missing token rejects streaming endpoints
- **WHEN** `accessControl.mode` is `token`
- **AND** `accessControl.token` is missing or empty
- **AND** a client attempts to connect to `/api/events` (SSE) or any `/api/**` WebSocket stream
- **THEN** the system rejects the connection
- **AND** the response is a standard `ApiResponse` error payload when applicable

