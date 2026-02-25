# access-control-boundary Specification

## Purpose
TBD - created by archiving change add-access-control-boundary. Update Purpose after archive.
## Requirements
### Requirement: Access-control configuration
The system SHALL expose `accessControl` configuration with:
- `mode`: `disabled | token`
- `token`: a shared secret string (used only when `mode=token`)
- `allowLocalhostBypass`: boolean defaulting to `true`

#### Scenario: Default access control
- **WHEN** `accessControl` is not configured
- **THEN** the system treats access control as `disabled` and allows requests without a token

#### Scenario: Default localhost bypass
- **WHEN** `accessControl.mode` is `token` and `allowLocalhostBypass` is unset
- **THEN** `allowLocalhostBypass` is treated as `true`

### Requirement: Disabled mode allows access
When access control mode is `disabled`, the system SHALL allow HTTP, SSE, and WebSocket requests without a token.

#### Scenario: Disabled HTTP access
- **WHEN** access control mode is `disabled`
- **THEN** `/api/**` requests succeed without a token

#### Scenario: Disabled streaming access
- **WHEN** access control mode is `disabled`
- **THEN** `/api/events` SSE and WebSocket streams succeed without a token

### Requirement: Token-protected API boundary
When access control mode is `token`, the system SHALL require a valid token for `/api/**` HTTP requests and SHALL return `401` with an `ApiResponse` error payload on unauthorized access. `/health` MUST remain public.

#### Scenario: Non-localhost requires token
- **WHEN** access control mode is `token` and `allowLocalhostBypass` is `false`
- **AND** a non-localhost `/api/**` request is missing a valid token
- **THEN** the system returns `401` with an `ApiResponse` error payload

#### Scenario: Localhost bypass applies
- **WHEN** access control mode is `token` and `allowLocalhostBypass` is `true`
- **AND** a localhost `/api/**` request provides no token
- **THEN** the request is accepted

#### Scenario: Non-localhost still requires token
- **WHEN** access control mode is `token` and `allowLocalhostBypass` is `true`
- **AND** a non-localhost `/api/**` request provides no token
- **THEN** the system returns `401` with an `ApiResponse` error payload

#### Scenario: Header token is accepted
- **WHEN** a `/api/**` request includes `Authorization: Bearer <token>`
- **OR** a `/api/**` request includes `X-API-Token: <token>`
- **THEN** the request is authorized when the token matches the configured token

#### Scenario: Token mismatch
- **WHEN** a `/api/**` request provides a token that does not match the configured token
- **THEN** the system returns `401` with an `ApiResponse` error payload

#### Scenario: Health remains public
- **WHEN** access control mode is `token`
- **THEN** `/health` requests succeed without a token

### Requirement: SSE and WebSocket token validation
When access control mode is `token`, the system SHALL require SSE and WebSocket streams to provide a valid token, and SHALL accept query parameters when headers cannot be set.

#### Scenario: SSE token via query param
- **WHEN** a client connects to `/api/events?token=<token>`
- **THEN** the SSE connection is accepted if the token is valid

#### Scenario: SSE token missing or invalid
- **WHEN** a client connects to `/api/events` with a missing or invalid token
- **THEN** the system returns `401` with an `ApiResponse` error payload

#### Scenario: WS token via query param
- **WHEN** a client opens a WebSocket connection with `?token=<token>`
- **THEN** the upgrade is accepted only if the token is valid

#### Scenario: WS token missing or invalid
- **WHEN** a client opens a WebSocket connection with a missing or invalid token
- **THEN** the connection is rejected and the server responds with `401`

### Requirement: Access-control response redaction
The system SHALL redact `accessControl.token` from any UserSystemInfo/config responses.

#### Scenario: Token is redacted
- **WHEN** a client requests UserSystemInfo/config
- **THEN** the response payload omits the `accessControl.token` value (empty or missing)

### Requirement: Frontend token passthrough
When a token is configured locally, the frontend SHALL attach the token to API, SSE, and WebSocket requests.

#### Scenario: Authorization header is attached
- **WHEN** the frontend has a local token
- **THEN** HTTP requests include `Authorization: Bearer <token>`

#### Scenario: Stream URLs include token
- **WHEN** the frontend has a local token
- **THEN** SSE and WebSocket URLs include `?token=<token>`

#### Scenario: No token means no injection
- **WHEN** the frontend has no local token
- **THEN** HTTP requests do not attach Authorization and stream URLs do not include `token`

