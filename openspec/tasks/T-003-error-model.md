# Task: T-003 Normalize API Error Model

## Background / Motivation
- Issue: P1-ERR-01
- Evidence: Mixed StatusCode returns and ApiResponse error payloads across routes.

## Scope
### In Scope
- Standardize ApiError -> StatusCode mapping.
- Migrate routes that return StatusCode directly to ApiError.
- Map DbErr::RecordNotFound and domain NotFound to 404.

### Out of Scope / Right Boundary
- API versioning.
- Internationalized error messages.

## Design
### Proposed
- Centralize status mapping in crates/server/src/error.rs.
- Replace ResponseJson(ApiResponse::error(...)) + 200 with 4xx/5xx.
- Provide consistent error_data/message usage.

### Alternatives Considered
- Leave endpoints as-is (keeps inconsistency).

## Change List
- crates/server/src/error.rs: expand mapping for NotFound and conflicts.
- crates/server/src/routes/projects.rs: return ApiError instead of StatusCode.
- crates/server/src/routes/filesystem.rs: return ApiError and 4xx status.
- crates/server/src/routes/task_attempts.rs: remove ad-hoc StatusCode returns.
- crates/server/src/routes/tasks.rs: adjust RecordNotFound mapping.
- Update frontend error handling if assumptions change.

## Acceptance Criteria
- NotFound returns 404 + ApiResponse error.
- BadRequest returns 400 + ApiResponse error.
- Conflict returns 409 + ApiResponse error.
- cargo test --workspace passes.

## Risks & Rollback
- Risk: frontend expects 200 with error payload.
- Rollback: restore previous response mapping or add compatibility parsing.

## Effort Estimate
- 1-2 days.

## Acceptance Scripts
```bash
export BACKEND_PORT=3001

# NotFound -> 404
curl -i "http://localhost:${BACKEND_PORT}/api/tasks/00000000-0000-0000-0000-000000000000"

# BadRequest -> 400 (invalid UUID)
curl -i "http://localhost:${BACKEND_PORT}/api/tasks/not-a-uuid"

# Filesystem missing path -> 404
curl -i "http://localhost:${BACKEND_PORT}/api/filesystem/directory?path=/path/does/not/exist"
```
Expected:
- 404 for missing resources with ApiResponse error payload.
- 400 for invalid parameters with ApiResponse error payload.
