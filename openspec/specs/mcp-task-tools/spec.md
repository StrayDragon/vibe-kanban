# mcp-task-tools Specification

## Purpose
将 Vibe Kanban 的 MCP 接口从“HTTP 代理”重写为本地优先的 control plane，并围绕 project/task/attempt + approvals + feed 提供稳定、可恢复、可高频调用的工具集。

## MODIFIED Requirements

### Requirement: MCP task tool set
The system SHALL expose a coherent MCP tool set for task, attempt, approvals, and activity operations with consistent naming and schemas.

The tool set SHALL include:
- Discovery: `list_projects`, `list_repos`, `list_executors`, `cli_dependency_preflight`
- Tasks: `list_tasks`, `get_task`, `create_task`, `update_task`, `delete_task`
- Attempts: `list_task_attempts`, `start_attempt`, `send_follow_up`, `stop_attempt`
- Observation: `tail_project_activity`, `tail_task_activity`, `tail_attempt_feed`, `tail_session_messages`
- Changes/artifacts: `get_attempt_changes`, `get_attempt_patch`, `get_attempt_file`
- Approvals: `list_approvals`, `get_approval`, `respond_approval`

#### Scenario: Tools are discoverable
- **WHEN** a client queries the MCP server for available tools
- **THEN** the tool list includes `tail_attempt_feed` and `respond_approval`

### Requirement: Start attempt is single-roundtrip for initial prompt
The system SHALL provide `start_attempt` that creates an attempt and ensures a session exists. If `prompt` is provided, the server SHALL enqueue/send it without requiring a separate follow-up call.

#### Scenario: Start attempt with prompt
- **WHEN** a client calls `start_attempt` with `prompt`
- **THEN** the response includes both `attempt_id` and `session_id` and the attempt begins producing logs

### Requirement: Consistent pagination semantics
MCP tools that return ordered history MUST distinguish “older paging” from “incremental tailing”:
- `cursor` means “page older history”
- `after_*` means “return only items newer than X”
- the server MUST reject requests that supply both `cursor` and `after_*`

#### Scenario: Reject mixed pagination modes
- **WHEN** a client calls any `tail_*` tool with both `cursor` and `after_*`
- **THEN** the server returns a structured tool error (`isError=true`) with `code=mixed_pagination` and a hint to use only one mode

## ADDED Requirements

### Requirement: MCP tools SHALL return structuredContent for JSON results
对于返回 JSON 的 MCP tools，系统 SHALL 在 tool result 中提供 `structuredContent`，并将其视为客户端机器消费的主通道。

#### Scenario: Structured output is present
- **WHEN** 客户端调用任一返回 JSON 的 tool（例如 `tail_attempt_feed` 或 `list_tasks`）
- **THEN** 返回结果包含非空 `structuredContent`，且其语义与 tool 文档一致

### Requirement: Tools SHOULD publish outputSchema for JSON results
对于返回 JSON 的 MCP tools，系统 SHOULD 在 tools/list 中提供 `outputSchema`，使客户端可进行 schema 驱动的解析与验证。

#### Scenario: Output schema is discoverable
- **WHEN** 客户端请求 MCP tool 列表
- **THEN** 关键 JSON tools（任务/attempt/approvals/feed 类）在 tool 定义中包含 `outputSchema`

### Requirement: Tools SHALL provide meaningful annotations
系统 SHALL 为 tools 提供 `annotations` 提示信息，至少覆盖：
- 只读工具标记 `readOnlyHint=true`
- 可能破坏性操作标记 `destructiveHint=true`
- 声明支持 `request_id` 幂等的写操作标记 `idempotentHint=true`

#### Scenario: Tools include annotations
- **WHEN** 客户端请求 MCP tool 列表
- **THEN** `annotations` 字段存在且与工具行为一致（只读/破坏性/幂等提示不互相矛盾）
