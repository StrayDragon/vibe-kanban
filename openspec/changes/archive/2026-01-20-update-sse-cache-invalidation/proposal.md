# Change: Reduce polling with SSE-driven cache invalidation

## Why
The UI currently polls task attempt and branch status data every 5 seconds, even when the backend already emits real-time events. This adds load and wastes client/network resources.

## What Changes
- Subscribe to the `/events` SSE stream on the frontend and use JSON Patch events to invalidate task attempt and branch status queries.
- Gate 5s polling behind SSE connectivity and page visibility so polling only runs as a fallback.
- Remove explicit 5s polling overrides where hooks already have smart fallback logic.

## Impact
- Affected specs: refresh-task-attempts (new)
- Affected code: `frontend/src/hooks/`, `frontend/src/components/`, `frontend/src/App.tsx` (or new provider/context)
- Behavior: task attempt + branch status freshness becomes event-driven with visibility-aware fallback polling
