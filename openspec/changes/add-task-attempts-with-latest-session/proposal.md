# Change: Add task attempts with latest session API

## Why
Fetching task attempts and their latest session currently triggers N+1 session fetches. A single backend API can reduce round-trips and latency.

## What Changes
- Add an API endpoint that returns task attempts with their latest session in one call.
- Update frontend hooks to use the new endpoint.
- Add tests and error handling for the new API surface.

## Impact
- Affected specs: task-attempts
- Affected code: crates/server/src/routes/task_attempts.rs, frontend/src/hooks/useTaskAttempts.ts, frontend/src/lib/api.ts
