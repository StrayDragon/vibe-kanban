# Change: Unify log persistence strategy for execution logs

## Why
The system currently double-writes raw logs to both JSONL (execution_process_logs) and indexed log entries (execution_process_log_entries), which increases I/O and storage while still relying on JSONL for legacy backfill. We need a single source of truth with explicit retention and a safe legacy backfill path.

## What Changes
- Use execution_process_log_entries as the canonical store for raw and normalized history.
- Stop writing JSONL for new executions when the log entry table is available; retain JSONL only as a legacy backfill source.
- Add a retention/cleanup policy for legacy JSONL rows to limit duplicate storage.
- Keep backfill compatibility for existing JSONL data so history endpoints remain complete.

## Impact
- Affected specs: execution-logs
- Affected code: crates/services/src/services/container.rs, crates/db/src/models/execution_process_logs.rs, crates/db/src/models/execution_process_log_entries.rs, crates/server/src/main.rs
