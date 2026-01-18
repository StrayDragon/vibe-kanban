# Task: T-007 Modularize Task Attempt Routes

## Background / Motivation
- Issue: P2-MOD-01
- Evidence: crates/server/src/routes/task_attempts.rs is 2000+ lines.

## Scope
### In Scope
- Split task_attempts routes by domain:
  - lifecycle (create/stop/remove)
  - logs/diff streams
  - git operations (rebase/push/merge)
  - setup/cleanup scripts
- Keep public API paths unchanged.

### Out of Scope / Right Boundary
- Behavior changes to API.
- Refactor of domain services.

## Design
### Proposed
- Create crates/server/src/routes/task_attempts/ submodules and re-export router.
- Keep shared helpers in task_attempts/util.rs.

## Change List
- crates/server/src/routes/task_attempts/*.rs
- crates/server/src/routes/task_attempts.rs (thin facade)

## Acceptance Criteria
- cargo check and tests pass.
- No API path changes.

## Risks & Rollback
- Low risk; structural refactor only.
- Rollback by reverting module split.

## Effort Estimate
- 1-2 days.

## Acceptance Scripts
```bash
rg --files crates/server/src/routes/task_attempts
cargo check --workspace
```
Expected:
- task_attempts submodules exist and compile cleanly.
