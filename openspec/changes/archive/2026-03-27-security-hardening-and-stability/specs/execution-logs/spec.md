# execution-logs Specification

## ADDED Requirements

### Requirement: Execution process APIs do not expose sensitive executor action content
The system SHALL NOT expose sensitive executor action content (for example, script bodies, secret-bearing arguments, or authorization headers) in `/api/execution-processes/**` response payloads.

The system MAY expose a minimal executor action summary sufficient for UI display (for example, action type and safe metadata), but MUST NOT include secret values.

#### Scenario: Get execution process does not include script bodies
- **WHEN** a client requests an execution process detail endpoint (for example `GET /api/execution-processes/{id}`)
- **THEN** the response payload does not include any executor action script body
- **AND** it does not include sensitive header values (for example `Authorization`)

