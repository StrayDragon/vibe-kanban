# mcp-activity-feed Specification

## Purpose
为外部编排器与人类接管场景提供 project/task/attempt 级别的增量活动拉取能力，并在 attempt 观测中支持低频调用与低延迟体验。

## Requirements

### Requirement: Project activity can be tailed incrementally
The system SHALL expose an MCP tool to tail project activity events. The tool SHALL support incremental polling via `after_event_id` and older paging via `cursor`.

This tool SHALL follow the canonical pagination semantics defined in `mcp-task-tools` (reject mixed pagination with `code=mixed_pagination`).

#### Scenario: Incremental project tail
- **WHEN** a client calls `tail_project_activity` with `after_event_id=X`
- **THEN** the response includes only events newer than X and returns `next_after_event_id`

### Requirement: Task activity can be tailed incrementally
The system SHALL expose an MCP tool to tail task activity events with the same pagination semantics as project activity.

#### Scenario: Tail task activity
- **WHEN** a client calls `tail_task_activity` for a task with recent changes
- **THEN** the response includes a bounded list of newest events in chronological order

### Requirement: Attempt feed returns latest logs plus pending approvals
The system SHALL expose an MCP tool to tail an attempt feed that can include:
- attempt state summary
- newest normalized log entries (bounded)
- pending approval summaries

The tool SHALL support incremental polling via `after_log_index`.

The tool SHALL support optional long-polling via `wait_ms` when `after_log_index` is provided:
- If there are no new log entries and no pending approvals at call time, the server SHALL wait up to `wait_ms` milliseconds for new logs or a pending approval to appear.
- The server SHALL return early when new logs/approvals are available.
- The server SHALL cap `wait_ms` and reject values above the cap with a structured tool error (`code=wait_ms_too_large`).

Invalid `wait_ms` usage (e.g. missing `after_log_index`) SHALL return a structured tool error per `api-error-model` (`code=wait_ms_requires_after_log_index`).

#### Scenario: Attempt feed includes pending approvals
- **WHEN** an attempt has pending approvals and a client calls `tail_attempt_feed`
- **THEN** the response includes approval summaries sufficient to prompt a user to approve/deny

#### Scenario: Attempt feed incremental logs
- **WHEN** a client calls `tail_attempt_feed` with `after_log_index=K`
- **THEN** the response includes only log entries newer than K and returns `next_after_log_index`

#### Scenario: Attempt feed long-polls for new logs
- **WHEN** a client calls `tail_attempt_feed` with `after_log_index=K` and `wait_ms=T` and there are no new logs yet
- **THEN** the server blocks for up to T milliseconds and returns as soon as a new log entry or a pending approval appears

## Compression Notes
- Former `Reject mixed pagination modes` scenario is covered by `mcp-task-tools` → `Consistent pagination semantics` and `api-error-model` (`code=mixed_pagination`).
- Former `wait_ms requires after_log_index` scenario is covered by `api-error-model` (`code=wait_ms_requires_after_log_index`) and the `Attempt feed` requirement above.
- Former `Feed and activity tools SHALL return structuredContent` requirement is covered by `mcp-task-tools` → `MCP tools SHALL return structuredContent for JSON results` and the per-tool scenarios in this spec that require `next_after_*` pointers.
- Former `Mixed pagination SHALL return a structured tool error` requirement is covered by `mcp-task-tools` pagination semantics plus `api-error-model` structured tool errors.
