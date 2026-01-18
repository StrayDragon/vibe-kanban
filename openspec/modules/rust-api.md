# Module: Rust API Layer

## Goals
- Standardize HTTP status codes across routes.
- Ensure ApiResponse is consistent and predictable.
- Reduce ad-hoc error handling in route handlers.

## In Scope
- ApiError mapping and conversions.
- Route handlers that return StatusCode directly.
- NotFound handling for DbErr::RecordNotFound and domain errors.

## Out of Scope / Right Boundary
- API versioning or breaking API contract changes.
- Full error localization.
- Large refactors of service logic.

## Design Summary
- Route handlers should return Result<ResponseJson<ApiResponse<T>>, ApiError>.
- ApiError -> StatusCode mapping rules:
  - BadRequest -> 400
  - Unauthorized -> 401
  - Forbidden -> 403
  - Conflict -> 409
  - RecordNotFound / ProjectNotFound / RepoNotFound -> 404
  - All other errors -> 500
- ApiResponse.error used for 4xx/5xx responses, not 200.
- SSE/WS handshake failures return 401/403 before upgrade.

## Testing
- Add tests asserting status code for common error cases.
- Update frontend tests if any error handling assumptions change.
