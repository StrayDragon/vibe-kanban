## 1. Cache inventory and sizing
- [ ] 1.1 Enumerate server caches and current sizing strategy.
- [ ] 1.2 Define default budgets and thresholds per cache.

## 2. Config and defaults
- [ ] 2.1 Add env/config wiring for cache budgets.
- [ ] 2.2 Document cache tuning defaults and guidance.

## 3. Apply budgets
- [ ] 3.1 Apply budgets to file search cache (Moka).
- [ ] 3.2 Bound file history/statistics cache (DashMap) with eviction or TTL.
- [ ] 3.3 Bound event stream history cache (MsgStore).
- [ ] 3.4 Add startup logging for cache budgets and sizes.
- [ ] 3.5 Add warning logs when caches exceed thresholds.

## 4. Tests
- [ ] 4.1 Add unit tests for cache eviction behavior where feasible.
- [ ] 4.2 Add tests for config defaults and overrides.
