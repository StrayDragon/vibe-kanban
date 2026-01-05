## Context
Auto-retry should be configurable per executor configuration and trigger when a coding-agent run fails with a recoverable error. The retry needs a delay and a max attempts limit, plus a visible system tip in the conversation.

## Goals / Non-Goals
- Goals:
  - Per-executor auto-retry settings in `/settings/agents`.
  - Regex-based error matching with configurable delay and max attempts.
  - Visible system tip when auto-retry is scheduled/executed.
- Non-Goals:
  - Cross-session persistence of retry counters across server restarts.
  - Adding new agent capabilities beyond retry behavior.

## Decisions
- Decision: Add an `auto_retry` object to executor configs (CodingAgent variants).
  - Fields: `error_patterns: string[]`, `delay_seconds: u64`, `max_attempts: u32`.
  - Disabled when `error_patterns` is empty or `max_attempts` is 0.

- Decision: Detect retry eligibility at process completion.
  - Trigger only when `ExecutionProcessStatus::Failed` and `run_reason == CodingAgent`.
  - Gather normalized error content (ErrorMessage + SystemMessage entries) and test regex list against the joined text.

- Decision: Schedule retry via a delayed task.
  - Use a lightweight in-memory tracker keyed by `(session_id, root_process_id)` to enforce `max_attempts` and avoid loops.
  - On trigger, enqueue a delayed follow-up that replays the original prompt and uses `retry_process_id` to restore the worktree to the failed process state.
  - Skip auto-retry if another process is already running for the session.

- Decision: Emit a system tip entry.
  - Push a `system_message` entry with metadata `{"system_tip":"auto_retry","attempt":N,"max":M,"delay_seconds":S}`.
  - UI renders this as a light-green tip card.

## Risks / Trade-offs
- In-memory counters reset on server restart; retry caps may reset.
- Regex performance: large pattern lists could be slow; compile-on-load and reuse in memory.
- Auto-retry could conflict with dirty worktrees; conservative defaults should avoid force-reset.

## Migration Plan
- Extend executor config schema + defaults.
- Add validation for regex patterns and numeric bounds.
- Implement auto-retry scheduling + system tip emission in container completion flow.
- Add UI styling for auto-retry tips.

## Open Questions
- Should auto-retry be skipped if worktree is dirty and reset is not allowed, or should it fall back to a normal follow-up without reset?
- Should the system tip appear on schedule, on execution, or both?
