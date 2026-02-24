## 0. Scope & Constraints
- Scope: Standardize error HTTP status codes while keeping success payload shapes unchanged.
- Non-goals: redesigning the success `ApiResponse` format; changing endpoint URLs; introducing a new error payload schema.

## 1. Backend: status mapping and handler cleanup
- [x] 1.1 Centralize and audit `ApiError -> StatusCode` mapping (at minimum: 400/401/403/404/409/500).
- [x] 1.2 Replace route-local `ApiResponse::error(...)` returns with `Err(ApiError::...)` where appropriate.
- [x] 1.3 Review endpoints using `error_with_data` and align their status semantics (e.g., `409` for conflicts).
- [x] 1.4 Ensure 5xx-class errors are logged via `tracing::error` with useful context.

## 2. Frontend: consistent error handling
- [x] 2.1 Update `frontend/src/api/client.ts` to consistently parse `ApiResponse` errors on non-2xx.
- [x] 2.2 Ensure callers handle non-2xx consistently (avoid assuming `ok=true` on HTTP 200 only).

## 3. Tests
- [x] 3.1 Unit test the `ApiError -> StatusCode` mapping.
- [x] 3.2 Add minimal route-level integration tests covering representative 400/404/409/500 behaviors.

## 4. Verification
- [x] 4.1 `cargo test --workspace`
- [x] 4.2 `pnpm -C frontend run test`
- [x] 4.3 `pnpm -C frontend run check`
- [x] 4.4 `pnpm -C frontend run lint`

## Acceptance Criteria
- All error responses return a non-200 HTTP status code.
- Error response bodies remain `ApiResponse` error envelopes and are readable by the existing frontend.
- Representative routes return correct codes for invalid input (400), missing resources (404), conflicts (409), and internal errors (500).
