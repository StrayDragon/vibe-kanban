# Module: Rust Domain Services

## Goals
- Keep multi-step workflows consistent and recoverable.
- Separate route wiring from domain orchestration.

## In Scope
- Task create + attempt create + start workflows.
- Workspace and repo creation in a transaction.
- Failure cleanup and rollback behavior.

## Out of Scope / Right Boundary
- New workflow engines or background job queues.
- Large refactor of container execution logic.
- Changing database schema for status tracking (unless required).

## Design Summary
- Transaction boundaries cover:
  - Task creation
  - Workspace creation
  - WorkspaceRepo creation
- Execution start happens after transaction commit.
- On start failure:
  - Remove workspace + repos (if safe), or
  - Mark attempt as failed via an execution process record (if available)
- Consolidate orchestration logic into a service helper to avoid duplication.

## Testing
- Unit tests or integration tests that simulate start failure and assert no orphan rows.
