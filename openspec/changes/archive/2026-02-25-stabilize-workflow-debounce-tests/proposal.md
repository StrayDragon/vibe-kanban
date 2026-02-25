# Change: Stabilize workflow debounce tests

## Why
Debounce-related tests currently rely on real-time sleeps, which can become flaky on loaded CI hosts and increase suite runtime.

## What Changes
- Replace real-time sleep-based debounce assertions with fake-timer driven assertions.
- Use deterministic timer advancement and `waitFor` for async UI updates.
- Keep runtime behavior unchanged; this change targets test determinism only.

## Impact
- Affected specs: `test-stability`
- Affected code: `frontend/src/pages/TaskGroupWorkflow/TaskGroupWorkflow.test.tsx` and related frontend tests
- Out of scope: functional workflow behavior changes.
