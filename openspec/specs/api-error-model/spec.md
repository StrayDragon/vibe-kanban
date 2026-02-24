# api-error-model Specification

## Purpose
TBD - created by archiving change add-api-error-model. Update Purpose after archive.
## Requirements
### Requirement: Consistent error status mapping
The system SHALL map `ApiError` variants to HTTP status codes and SHALL return an `ApiResponse` error payload under a non-200 status.

#### Scenario: BadRequest status
- **WHEN** input validation fails
- **THEN** the response status is `400` and the body is an `ApiResponse` error payload

#### Scenario: Unauthorized status
- **WHEN** a request is missing required authentication
- **THEN** the response status is `401` and the body is an `ApiResponse` error payload

#### Scenario: Forbidden status
- **WHEN** a request is authenticated but not permitted
- **THEN** the response status is `403` and the body is an `ApiResponse` error payload

#### Scenario: NotFound status
- **WHEN** the requested resource does not exist
- **THEN** the response status is `404` and the body is an `ApiResponse` error payload

#### Scenario: Conflict status
- **WHEN** the request conflicts with existing state
- **THEN** the response status is `409` and the body is an `ApiResponse` error payload

#### Scenario: Internal server error status
- **WHEN** an unexpected server error occurs
- **THEN** the response status is `500` and the body is an `ApiResponse` error payload

