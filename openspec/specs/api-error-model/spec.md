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

## ADDED Requirements (MCP)

### Requirement: Business failures SHOULD be returned as structured tool errors
对于可恢复或业务语义明确的失败（例如混用分页、幂等冲突、guardrails 阻断），系统 SHOULD 返回 tool-level error（`isError=true`）并提供结构化错误对象，至少包含：
- `code`（稳定字符串）
- `retryable`（布尔值）
- `hint`（下一步建议）
- `details`（上下文）

#### Scenario: Tool errors are structured
- **WHEN** 客户端触发一次业务失败（例如混用分页）
- **THEN** 返回 `isError=true` 且错误内容包含 `code/hint`，客户端无需解析字符串 JSON 即可编排恢复动作
