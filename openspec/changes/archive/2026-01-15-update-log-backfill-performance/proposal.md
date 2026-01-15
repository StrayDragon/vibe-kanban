# Change: Update log backfill performance

## Why
Legacy JSONL backfill runs synchronously at startup and reprocesses logs on each restart, which can delay service readiness and add avoidable IO.

## What Changes
- Run legacy JSONL backfill asynchronously with bounded concurrency.
- Track backfill completion in a bounded, TTL cache to prevent unbounded memory growth.
- Skip backfill work when log entries already exist and no backfill is needed.

## Impact
- Affected specs: execution-logs
- Affected code: crates/services/src/services/container.rs, crates/services/src/services/cache_budget.rs
