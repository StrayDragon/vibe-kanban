# mcp-activity-feed Specification

## Purpose
让 attempt/project/task 级别的 feed/activity 在高频轮询下仍保持稳定的机器可读语义，并提供一致的错误与分页契约。

## ADDED Requirements

### Requirement: Feed and activity tools SHALL return structuredContent
系统 SHALL 为 `tail_attempt_feed/tail_project_activity/tail_task_activity` 返回 `structuredContent`，并保证字段与分页语义可被客户端直接消费。

#### Scenario: Incremental tail returns structured content
- **WHEN** 客户端使用 `after_log_index` 或 `after_event_id` 增量拉取
- **THEN** 返回包含 `structuredContent`，并提供可用于下一次增量拉取的 `next_after_*` 指针

### Requirement: Mixed pagination SHALL return a structured tool error
当客户端同时提供 `cursor` 与 `after_*`（混用分页模式）时，系统 SHALL 返回 tool-level error，且错误对象为结构化内容（包含 `code` 与 `hint`）。

#### Scenario: Reject mixed pagination modes with structured error
- **WHEN** 客户端调用 `tail_*` 工具并同时提供 `cursor` 与 `after_*`
- **THEN** tool result 的 `isError=true`，并在结构化错误对象中包含 `code=mixed_pagination` 与可操作的 `hint`

