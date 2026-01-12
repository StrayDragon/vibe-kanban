## Context
Execution logs are persisted in two places today:
- execution_process_logs: JSONL rows appended for stdout/stderr (plus a few special cases)
- execution_process_log_entries: indexed entries for raw + normalized history used by v2 endpoints

This creates duplicate I/O and storage. JSONL is still used as the legacy backfill source for log entry history, and startup backfill scans all JSONL rows to rebuild entries.

## Goals / Non-Goals
- Goals:
  - Make log_entries the single source of truth for persisted history.
  - Reduce duplicate DB writes for raw logs.
  - Preserve backfill compatibility for legacy JSONL data.
  - Define a retention policy for legacy JSONL rows.
- Non-Goals:
  - Change client-facing log history APIs or UI behavior.
  - Alter log entry indexing semantics or in-memory history limits.

## Decisions
- Canonical store: execution_process_log_entries is the authoritative persistence for raw and normalized history.
- JSONL writes: stop writing JSONL for new executions when log_entries are available; keep JSONL only as a legacy backfill source.
- Backfill: continue backfilling log_entries from JSONL for executions that still have legacy rows.
- Retention: delete legacy JSONL rows for completed executions after a configurable retention window; allow disabling cleanup via config.

## Alternatives considered
- Keep dual-write indefinitely: simple but keeps double I/O and storage costs.
- Make JSONL canonical and derive log_entries lazily: avoids some duplication but keeps slower backfill and larger reads for history.
- Move JSONL to file storage: reduces DB size but adds new storage surface area and operational complexity.

## Risks / Trade-offs
- Reduced redundancy: if log_entries ingestion fails, there is no JSONL fallback for new executions. Mitigation: keep retry logic, add diagnostics, and allow opting into legacy mode.
- Cleanup safety: aggressive JSONL deletion could remove backfill sources. Mitigation: enforce retention window and require completed executions.

## Migration Plan
1) Introduce log persistence mode detection (auto if log_entries table exists; optional override).
2) Deploy with log_entries-only persistence for new executions.
3) Keep startup/on-demand backfill from JSONL for existing executions.
4) Add retention cleanup for legacy JSONL rows after the configured window.

## Open Questions
- What should the default retention window be (e.g., 7 or 14 days)?
- Do we need a diagnostic metric to flag missing log_entries for completed executions?
