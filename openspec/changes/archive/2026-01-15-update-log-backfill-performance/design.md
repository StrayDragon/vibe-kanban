## Context
Startup backfill currently parses all legacy JSONL logs synchronously and repeats on every restart. The backfill completion cache is unbounded, which can grow indefinitely in long-running services.

## Goals / Non-Goals
- Goals:
  - Avoid blocking service readiness on legacy backfill.
  - Bound memory used for backfill completion tracking.
  - Preserve correct log history after backfill.
- Non-Goals:
  - Changing log storage formats.
  - Removing legacy JSONL support.

## Decisions
- Decision: Run legacy backfill as a background task with bounded concurrency.
- Decision: Track per-execution/channel backfill completion in a bounded cache with TTL, using cache budget configuration.
- Decision: Keep existing correctness checks for whether backfill is needed, but avoid reprocessing if entries already exist.

## Risks / Trade-offs
- Background backfill means some history may still be served from legacy logs briefly after startup.
- Cache TTL expiration can trigger reprocessing; keep TTL high enough to avoid churn.

## Migration Plan
- Add new cache budget env vars for backfill completion.
- Replace the static set with an expiring cache implementation.
- Move startup backfill into a background task and cap concurrency.
- Add tests for non-blocking behavior and cache eviction.

## Open Questions
- What default TTL and max entries should be used for backfill completion caching?
- Do we need a persistent backfill marker to avoid reprocessing after a restart?
