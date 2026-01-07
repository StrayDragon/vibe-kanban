# Change: Add server cache budgeting

## Why
Server memory can grow due to long-lived in-process caches (event stream history, file search cache, repo history cache, and other runtime caches). These caches are either unbounded or have static limits today, which makes memory usage unpredictable under heavy workloads.

## What Changes
- Define configurable budgets (entry count + TTL) for server caches and enforce eviction policies.
- Add startup logging that summarizes cache budgets and current sizes.
- Add warning logs when caches exceed configured thresholds.
- Document cache tuning defaults and how to adjust them.
- Defer event stream MsgStore budgeting to update-log-history-streaming to avoid duplication.

## Impact
- Affected specs: cache-budgeting
- Affected code:
  - crates/services/src/services/events.rs
  - crates/services/src/services/file_search_cache.rs
  - crates/services/src/services/file_ranker.rs
  - crates/utils/src/msg_store.rs (if shared budget configuration is applied)
  - crates/services/src/services/config/versions (new config/env wiring)
