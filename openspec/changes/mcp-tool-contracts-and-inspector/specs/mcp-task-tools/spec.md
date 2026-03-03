# mcp-task-tools Specification (Incremental)

## MODIFIED Requirements

### Requirement: Tools SHOULD publish outputSchema for JSON results
对于返回 JSON 的 MCP tools，系统 SHALL 在 `tools/list` 中提供 `outputSchema`，使客户端可进行 schema 驱动的解析与验证。

`outputSchema` 的语义 SHALL 与 tool 成功返回时的 `structuredContent` 对齐（即：schema 描述的对象结构与字段应与 `structuredContent` 一致）。

#### Scenario: Output schema is discoverable for all tools in the declared set
- **WHEN** 客户端请求 MCP tool 列表（`tools/list`）
- **THEN** `mcp-task-tools` 规定的 tool set 中每一个 tool 定义都包含非空 `outputSchema`

## ADDED Requirements

### Requirement: Slow/heavy tools SHALL declare taskSupport
系统 SHALL 为“慢/重/可取消”的 tools 在 `tools/list` 中声明 `execution.taskSupport=optional`，以允许客户端通过 `tasks/*` 闭环执行并轮询/取消这些调用。

最低覆盖集合 SHALL 包含：
- `get_attempt_changes`
- `get_attempt_patch`
- `get_attempt_file`
- `start_attempt`
- `send_follow_up`
- `stop_attempt`

#### Scenario: start_attempt is task-capable
- **WHEN** 客户端请求 MCP tool 列表（`tools/list`）
- **THEN** `start_attempt` 的 tool 定义包含 `execution.taskSupport=optional`

#### Scenario: changes/artifacts tools are task-capable
- **WHEN** 客户端请求 MCP tool 列表（`tools/list`）
- **THEN** `get_attempt_changes/get_attempt_patch/get_attempt_file` 的 tool 定义包含 `execution.taskSupport=optional`
