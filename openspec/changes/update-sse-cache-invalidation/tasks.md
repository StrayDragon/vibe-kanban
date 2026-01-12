## 1. Implementation
- [ ] 1.1 Add an SSE client/provider for `/events` that tracks connectivity and invalidates React Query caches on JSON Patch events.
- [ ] 1.2 Add visibility-aware polling helper(s) and refactor `useTaskAttempts`, `useTaskAttemptsWithSessions`, and `useBranchStatus` to disable 5s polling when SSE is connected.
- [ ] 1.3 Remove redundant per-call 5s polling overrides (e.g., `CreateAttemptDialog`) and rely on hook-level fallback behavior.
- [ ] 1.4 Add lightweight tests for the interval/patch-invalidation helpers.
