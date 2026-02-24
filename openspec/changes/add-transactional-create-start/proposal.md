# Change: Add transactional create/start (tasks + attempts)

## Why
- Current multi-step create/start flows can leave partial DB state on failure.
- `start_workspace` failures after DB writes can leave orphaned `workspace` / `workspace_repo` records.

## What Changes
- Wrap task + attempt creation writes in a DB transaction where applicable.
- Ensure post-commit failures (e.g. workspace start) trigger idempotent cleanup of created workspace records before returning the error.
- Add logs for rollback/cleanup to aid debugging.

## Impact
- New spec: `transactional-create-start`.
- Code areas: server route handlers and services involved in create/start; DB helpers; tests.
- Compatibility: API shapes remain stable; only failure behavior becomes more consistent (no partial records).

