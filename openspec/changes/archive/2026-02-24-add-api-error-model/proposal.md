# Change: Add API error model (status codes + ApiResponse)

## Why
- The backend currently mixes `200 + ApiResponse::error(...)` with proper 4xx/5xx responses.
- Inconsistent status codes complicate frontend behavior, debugging, and automation.

## What Changes
- Define a canonical mapping from `ApiError` variants to HTTP status codes (4xx/5xx).
- Ensure error responses use **non-200** HTTP status codes while preserving the existing `ApiResponse` error envelope.
- Update route handlers to return `Err(ApiError::...)` rather than manually returning `ApiResponse::error(...)`.
- Frontend: ensure the API client consistently treats non-2xx responses as failures and surfaces `ApiResponse` error details.

## Impact
- New spec: `api-error-model`.
- Code areas: `crates/server/src/error.rs`, route handlers under `crates/server/src/routes/**`, frontend API client (`frontend/src/api/client.ts`), and tests.
- Compatibility: this is a breaking change for any external client that assumes errors still return `200`. Our frontend must be updated in the same change.

