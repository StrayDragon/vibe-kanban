# mcp-task-tools Specification

## Purpose
将 Vibe Kanban 的 MCP 接口作为外部编排器（OpenClaw 类）的 control plane，并围绕 project/task/attempt + approvals + feed 提供稳定、可恢复、可高频调用的工具集。

## Requirements

### Requirement: MCP task tool set
The system SHALL expose a coherent MCP tool set for task, attempt, approvals, and activity operations with consistent naming and schemas.

The tool set SHALL include:
- Discovery: `list_projects`, `list_repos`, `list_executors`, `cli_dependency_preflight`
- Tasks: `list_tasks`, `get_task`, `create_task`, `update_task`, `delete_task`
- Attempts: `list_task_attempts`, `start_attempt`, `send_follow_up`, `stop_attempt`
- Attempt control (lease): `claim_attempt_control`, `get_attempt_control`, `release_attempt_control`
- Observation: `tail_project_activity`, `tail_task_activity`, `tail_attempt_feed`, `tail_session_messages`
- Changes/artifacts: `get_attempt_changes`, `get_attempt_patch`, `get_attempt_file`
- Approvals: `list_approvals`, `get_approval`, `respond_approval`

#### Scenario: Tools are discoverable
- **WHEN** a client queries the MCP server for available tools
- **THEN** the tool list includes `tail_attempt_feed`, `respond_approval`, and `claim_attempt_control`

### Requirement: Start attempt is single-roundtrip for initial prompt
The system SHALL provide `start_attempt` that creates an attempt and ensures a session exists. If `prompt` is provided, the server SHALL enqueue/send it without requiring a separate follow-up call.

`start_attempt` SHALL return a `control_token` representing a lease for mutating attempt operations.

#### Scenario: Start attempt with prompt
- **WHEN** a client calls `start_attempt` with `prompt`
- **THEN** the response includes `attempt_id`, `session_id`, and `control_token` and the attempt begins producing logs

### Requirement: Consistent pagination semantics
MCP tools that return ordered history MUST distinguish “older paging” from “incremental tailing”:
- `cursor` means “page older history”
- `after_*` means “return only items newer than X”
- the server MUST reject requests that supply both `cursor` and `after_*`

#### Scenario: Reject mixed pagination modes
- **WHEN** a client calls any `tail_*` tool with both `cursor` and `after_*`
- **THEN** the server returns a structured tool error per `api-error-model` (`isError=true`, `code=mixed_pagination`) and a hint to use only one mode

### Requirement: Mutating attempt tools SHALL require a valid control_token
The system SHALL require a valid `control_token` for mutating attempt operations such as `send_follow_up` and `stop_attempt`.

#### Scenario: Follow-up requires control token
- **WHEN** a client calls `send_follow_up` without a valid `control_token`
- **THEN** the server returns a structured tool error per `api-error-model` with `code=invalid_control_token` or `code=attempt_claim_required`

### Requirement: Attempt control lease can be claimed and released
The system SHALL expose tools to claim, read, and release attempt control. A claim SHALL have a TTL and SHALL be reclaimable after expiry.

#### Scenario: Claim conflicts when lease is held
- **WHEN** a client calls `claim_attempt_control` for an attempt with an unexpired lease held by another client
- **THEN** the server returns a structured tool error (`isError=true`) with `code=attempt_claim_conflict` and a hint describing the current owner and expiry

#### Scenario: Lease can be released
- **WHEN** a client calls `release_attempt_control` with the matching `control_token`
- **THEN** the attempt becomes claimable by another client

### Requirement: MCP tools SHALL return structuredContent for JSON results
对于返回 JSON 的 MCP tools，系统 SHALL 在 tool result 中提供 `structuredContent`，并将其视为客户端机器消费的主通道。

#### Scenario: Structured output is present
- **WHEN** 客户端调用任一返回 JSON 的 tool（例如 `tail_attempt_feed` 或 `list_tasks`）
- **THEN** 返回结果包含非空 `structuredContent`，且其语义与 tool 文档一致

### Requirement: Tools SHALL publish outputSchema for JSON results
对于返回 JSON 的 MCP tools，系统 SHALL 在 `tools/list` 中提供 `outputSchema`，使客户端可进行 schema 驱动的解析与验证。

`outputSchema` 的语义 SHALL 与 tool 成功返回时的 `structuredContent` 对齐（即：schema 描述的对象结构与字段应与 `structuredContent` 一致）。

#### Scenario: Output schema is discoverable for all tools in the declared set
- **WHEN** 客户端请求 MCP tool 列表（`tools/list`）
- **THEN** `mcp-task-tools` 规定的 tool set 中每一个 tool 定义都包含非空 `outputSchema`

### Requirement: Slow/heavy tools SHALL declare taskSupport
系统 SHALL 为“慢/重/可取消”的 tools 在 `tools/list` 中声明 `execution.taskSupport=optional`，以允许客户端通过 `tasks/*` 闭环执行并轮询/取消这些调用。

最低覆盖集合 SHALL 包含：
- `get_attempt_changes`
- `get_attempt_patch`
- `get_attempt_file`
- `start_attempt`
- `send_follow_up`
- `stop_attempt`

#### Scenario: slow/heavy tools are task-capable
- **WHEN** 客户端请求 MCP tool 列表（`tools/list`）
- **THEN** `start_attempt/send_follow_up/stop_attempt/get_attempt_changes/get_attempt_patch/get_attempt_file` 的 tool 定义包含 `execution.taskSupport=optional`

### Requirement: Tools SHALL provide meaningful annotations
系统 SHALL 为 tools 提供 `annotations` 提示信息，至少覆盖：
- 只读工具标记 `readOnlyHint=true`
- 可能破坏性操作标记 `destructiveHint=true`
- 声明支持 `request_id` 幂等的写操作标记 `idempotentHint=true`

#### Scenario: Tools include annotations
- **WHEN** 客户端请求 MCP tool 列表
- **THEN** `annotations` 字段存在且与工具行为一致（只读/破坏性/幂等提示不互相矛盾）
