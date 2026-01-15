## 1. Implementation
- [x] 1.1 Add cache budget controls for log backfill completion tracking (TTL + max entries)
- [x] 1.2 Replace the unbounded backfill-complete set with a bounded, expiring cache
- [x] 1.3 Move startup backfill to a background task and cap concurrency
- [x] 1.4 Add tests for non-blocking startup and cache eviction behavior
- [x] 1.5 Update logging/metrics to show backfill progress and completion
