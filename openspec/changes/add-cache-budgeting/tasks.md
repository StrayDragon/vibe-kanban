## 1. Cache inventory and sizing
- [x] 1.1 Enumerate server caches and current sizing strategy.
- [x] 1.2 Define default budgets and thresholds per cache.

## 2. Config and defaults
- [x] 2.1 Add env/config wiring for cache budgets.
- [x] 2.2 Document cache tuning defaults and guidance.

## 3. Apply budgets
- Note: Event stream MsgStore budgets are deferred to update-log-history-streaming.
- [x] 3.1 Apply budgets to file search cache (Moka).
- [x] 3.2 Bound file history/statistics cache (DashMap) with eviction or TTL.
- [x] 3.3 Add startup logging for cache budgets and sizes.
- [x] 3.4 Add warning logs when caches exceed thresholds.

## 4. Tests
- [x] 4.1 Add unit tests for cache eviction behavior where feasible.
- [x] 4.2 Add tests for config defaults and overrides.
