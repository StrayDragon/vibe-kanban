# Task: T-004 Transactional Create/Start Flows

## Background / Motivation
- Issue: P1-DATA-01
- Evidence: create_task_and_start and create_task_attempt perform multi-step writes without rollback.

## Scope
### In Scope
- Wrap DB writes (Task, Workspace, WorkspaceRepo) in a transaction.
- Handle start_workspace failure with cleanup or compensating delete.

### Out of Scope / Right Boundary
- New background job system.
- Schema changes for explicit attempt status.

## Design
### Proposed
- Use a transaction for DB writes in:
  - crates/server/src/routes/tasks.rs:create_task_and_start
  - crates/server/src/routes/task_attempts.rs:create_task_attempt
- After commit, call start_workspace.
- On start_workspace failure:
  - Delete workspace + workspace_repos (if created in the same flow), or
  - Return error and log cleanup failures.
- Centralize shared logic in a service helper to avoid duplication.

### Alternatives Considered
- Start execution before commit (risk of partial DB state).
- Add new status fields (deferred).

## Change List
- crates/server/src/routes/tasks.rs: transaction + rollback handling.
- crates/server/src/routes/task_attempts.rs: transaction + rollback handling.
- crates/services/src/services/container.rs: optional helper for prompt override if needed.

## Acceptance Criteria
- When start_workspace fails, no new workspace or workspace_repo rows remain.
- Task creation failure does not create any partial records.
- cargo test --workspace passes.

## Risks & Rollback
- Risk: cleanup may fail on filesystem or external state.
- Rollback: revert to previous flow or gate new behavior behind a feature flag.

## Effort Estimate
- 2-3 days.

## Acceptance Scripts
```bash
# Runs new rollback tests (to be added with this task)
cargo test -p server transactional_create_start_rolls_back

# Full suite (optional)
cargo test --workspace
```
Expected:
- Rollback test passes and verifies no orphan workspace/workspace_repo rows.
