# mcp-activity-feed Specification

## Purpose
为外部编排器与人类接管场景提供 project/task/attempt 级别的增量活动拉取能力，并在 attempt 观测中支持低频调用与低延迟体验。

## MODIFIED Requirements

### Requirement: Attempt feed returns latest logs plus pending approvals
The system SHALL expose an MCP tool to tail an attempt feed that can include:
- attempt state summary
- newest normalized log entries (bounded)
- pending approval summaries

The tool SHALL support incremental polling via `after_log_index`.

The tool SHALL support optional long-polling via `wait_ms` when `after_log_index` is provided:
- If there are no new log entries and no pending approvals at call time, the server SHALL wait up to `wait_ms` milliseconds for new logs or a pending approval to appear.
- The server SHALL return early when new logs/approvals are available.
- The server SHALL cap `wait_ms` and reject values above the cap with a structured tool error.

#### Scenario: Attempt feed includes pending approvals
- **WHEN** an attempt has pending approvals and a client calls `tail_attempt_feed`
- **THEN** the response includes approval summaries sufficient to prompt a user to approve/deny

#### Scenario: Attempt feed incremental logs
- **WHEN** a client calls `tail_attempt_feed` with `after_log_index=K`
- **THEN** the response includes only log entries newer than K and returns `next_after_log_index`

#### Scenario: Attempt feed long-polls for new logs
- **WHEN** a client calls `tail_attempt_feed` with `after_log_index=K` and `wait_ms=T` and there are no new logs yet
- **THEN** the server blocks for up to T milliseconds and returns as soon as a new log entry appears

#### Scenario: wait_ms requires after_log_index
- **WHEN** a client calls `tail_attempt_feed` with `wait_ms` but without `after_log_index`
- **THEN** the server returns a structured tool error (`isError=true`) with `code=wait_ms_requires_after_log_index`

