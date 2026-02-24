## 0. Scope & Constraints
- Scope: transactional consistency for the core create/start flows:
  - `create_task_and_start`
  - `create_task_attempt` (and any helper that creates workspace records)
- Non-goals: redesigning orchestration; changing endpoint URLs; adding new DB tables.

## 1. Transactions
- [ ] 1.1 Make `create_task_and_start` write `task`, `task_image`, `workspace`, and `workspace_repo` in a single transaction.
- [ ] 1.2 Make `create_task_attempt` write `workspace` and `workspace_repo` in a single transaction.

## 2. Post-commit failure cleanup
- [ ] 2.1 When `start_workspace` fails after a successful commit, clean up the created `workspace` and `workspace_repo` records before returning the error (task/attempt records may remain).
- [ ] 2.2 Extract an idempotent cleanup helper (safe to call multiple times).
- [ ] 2.3 Add structured logs for rollback and cleanup failures.

## 3. Tests
- [ ] 3.1 Add a test: create failure rolls back (no partial `task/workspace/workspace_repo` leftovers).
- [ ] 3.2 Add a test: start failure cleans workspace records (`workspace/workspace_repo` removed).

## 4. Verification
- [ ] 4.1 `cargo test --workspace`
- [ ] 4.2 `pnpm -C frontend run check`

## Acceptance Criteria
- Any write failure during create/start does not leave partial DB records.
- Any `start_workspace` failure after commit results in workspace records being cleaned up before the API returns an error.
- Rollback/cleanup behavior is visible via logs and enforced by automated tests.

