## Context
Several server-side caches retain data in memory to improve UX and performance. Examples include:
- Event stream history stored in an in-memory MsgStore.
- File search cache (Moka) with a static 100MB cap.
- File history/statistics cache (DashMap) without a clear size bound.
These caches can grow or remain hot in long-running sessions, leading to high memory use.

## Current Cache Inventory (as of this change)
- Event stream history: `crates/services/src/services/events.rs` + `crates/utils/src/msg_store.rs`
  - Uses MsgStore with fixed byte limit; history is cloned per stream subscriber.
- File search cache: `crates/services/src/services/file_search_cache.rs`
  - Moka cache with `max_capacity(50)` (repo count, not bytes) + 1h TTL.
  - Each entry holds FST index + full file list; memory can be large per repo.
  - File watchers are stored in a DashMap and not currently pruned.
- File history stats cache: `crates/services/src/services/file_ranker.rs`
  - Global DashMap without TTL or size bound.
- Approvals cache: `crates/services/src/services/approvals.rs`
  - Pending and completed DashMaps; completed entries are never pruned.
- Queued follow-up messages: `crates/services/src/services/queued_message.rs`
  - DashMap with one message per session; usually small but unbounded.
- Executor profiles cache: `crates/executors/src/profile.rs`
  - Global LazyLock cache; bounded by config file size (low risk).
- Misc small caches: `crates/utils/src/lib.rs` (WSL2 detection), `crates/services/src/services/notification.rs` (WSL root path).

## Background Summary (from investigation)
- Event MsgStore history is cloned per subscriber, so memory grows with both history size and concurrent connections.
- File search cache uses `max_capacity(50)` repos, not bytes; each repo entry can be large (FST + full file list).
- File search watchers are stored in memory and never pruned.
- File stats cache (DashMap) is unbounded and never pruned.
- Approvals "completed" cache has no cleanup and grows over time.
- Queued follow-up messages are per-session and typically small but unbounded.
- JSONL log parsing is a major memory spike but is handled by update-log-history-streaming, not this change.

## Goals / Non-Goals
- Goals:
  - Make cache sizes predictable via configurable budgets.
  - Enforce eviction policies when budgets are exceeded.
  - Surface cache budget and size visibility via startup logs.
  - Keep behavior compatible with existing callers.
- Non-Goals:
  - Redesigning the log streaming pipeline (handled in update-log-history-streaming).
  - Introducing external cache services.
  - UI changes.

## Decisions
- Decision: Cache budgets via configuration
  - Provide environment/config values for each major cache budget.
  - Defaults are conservative and documented.
- Decision: Eviction policies
  - Use LRU/TTL where available (Moka) and implement bounded structures for custom caches.
  - For DashMap-based caches, migrate to a bounded strategy or add periodic pruning.
- Decision: Observability
  - Log cache budgets and current size at startup.
  - Emit warnings when caches exceed thresholds or prune aggressively.

## Proposed Default Budgets (initial)
These are starting defaults intended to be safe under moderate load; they should be tuned per deployment.
- Event stream MsgStore:
  - `EVENTS_MSGSTORE_MAX_BYTES=2mb`
  - `EVENTS_MSGSTORE_MAX_ENTRIES=2000`
- File search cache (Moka):
  - `FILE_SEARCH_CACHE_MAX_BYTES=128mb` (use weighted size; fall back to entry count if needed)
  - `FILE_SEARCH_CACHE_MAX_REPOS=25` (fallback if byte weigher is unavailable)
  - `FILE_SEARCH_CACHE_TTL_SECS=3600`
  - `FILE_SEARCH_WATCHERS_MAX=25` and `FILE_SEARCH_WATCHER_TTL_SECS=21600`
- File history stats cache (DashMap):
  - `FILE_STATS_CACHE_MAX_REPOS=25`
  - `FILE_STATS_CACHE_TTL_SECS=3600`
- Approvals cache:
  - `APPROVALS_COMPLETED_TTL_SECS=86400`
- Queued follow-up messages:
  - `QUEUED_MESSAGES_TTL_SECS=86400`
- Warning thresholds:
  - `CACHE_WARN_AT_RATIO=0.9` (log warnings at 90% of budget)

## Risks / Trade-offs
- More configuration increases operational complexity.
- Aggressive eviction could reduce cache hit rate and performance.
- Pruning large caches at startup can add latency.

## Migration Plan
- Add new config/env values with safe defaults.
- Apply budgets to caches incrementally, starting with the largest memory offenders.
- Update docs and tuning guidance.

## Suggested Worktree Split (implementation planning)
- Worktree A: config/env wiring + startup budget logging + warning thresholds.
- Worktree B: file_search_cache weighting/TTL + watcher pruning.
- Worktree C: file_stats_cache bounds (TTL/LRU) + pruning job.
- Worktree D: approvals/completed TTL cleanup + queued message TTL.
- Worktree E: event stream MsgStore budgets + stream history reduction.

## Open Questions
- Validate default budgets in production-like workloads.
- Whether to expose cache metrics via a health/status endpoint.
