# mcp-activity-feed Specification

## Purpose
为外部编排器与人类接管场景提供 project/task/attempt 级别的增量活动拉取能力，避免客户端通过多次调用自行拼装“发生了什么”。

## ADDED Requirements

### Requirement: Project activity can be tailed incrementally
The system SHALL expose an MCP tool to tail project activity events. The tool SHALL support incremental polling via `after_event_id` and older paging via `cursor`.

#### Scenario: Incremental project tail
- **WHEN** a client calls `tail_project_activity` with `after_event_id=X`
- **THEN** the response includes only events newer than X and returns `next_after_event_id`

#### Scenario: Reject mixed pagination modes
- **WHEN** a client calls `tail_project_activity` with both `after_event_id` and `cursor`
- **THEN** the server returns an invalid-argument error with a hint to use only one mode

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

#### Scenario: Attempt feed includes pending approvals
- **WHEN** an attempt has pending approvals and a client calls `tail_attempt_feed`
- **THEN** the response includes approval summaries sufficient to prompt a user to approve/deny

#### Scenario: Attempt feed incremental logs
- **WHEN** a client calls `tail_attempt_feed` with `after_log_index=K`
- **THEN** the response includes only log entries newer than K and returns `next_after_log_index`

