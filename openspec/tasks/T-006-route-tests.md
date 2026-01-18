# Task: T-006 Add Route-Level Tests

## Background / Motivation
- Issue: P1-TEST-01
- Evidence: No crates/server/tests present for critical API flows.

## Scope
### In Scope
- Add minimal integration tests for key routes:
  - /api/tasks (create, get)
  - /api/task-attempts (create)
  - /api/events (SSE auth boundary)
  - /api/info (auth boundary)

### Out of Scope / Right Boundary
- Full end-to-end browser tests.
- Large fixture data sets.

## Design
### Proposed
- Use axum Router with DeploymentImpl for tests.
- Use sqlite in-memory or temp DB with migrations.
- Keep tests isolated and deterministic.

## Change List
- crates/server/tests/routes_tasks.rs
- crates/server/tests/routes_task_attempts.rs
- crates/server/tests/routes_auth.rs
- Test helpers for setup (deployment + db)

## Acceptance Criteria
- cargo test --workspace passes.
- Auth tests assert 401 when token required.

## Risks & Rollback
- Low risk; tests only.

## Effort Estimate
- 1-2 days.

## Acceptance Scripts
```bash
cargo test -p server --tests
```
Expected:
- New route-level tests pass and cover auth + task creation flows.
