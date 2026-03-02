# mcp-approvals Specification

## Purpose
在 approvals 闭环中引入结构化输出与标准交互能力，使外部编排器可以稳定展示审批内容、记录审计信息，并在支持时通过 elicitation 降低轮询成本。

## ADDED Requirements

### Requirement: Approvals tools SHALL return structuredContent
系统 SHALL 为 approvals 相关 tools（`list_approvals/get_approval/respond_approval`）返回 `structuredContent`，并确保包含渲染审批 UI 所需的字段（tool_name/tool_input/tool_call_id/timestamps 等）。

#### Scenario: Approval details are machine-readable
- **WHEN** 客户端调用 `get_approval`
- **THEN** 返回包含 `structuredContent`，且其中包含 `tool_name` 与 `tool_input`

### Requirement: Approval responses SHOULD capture responder identity
系统 SHOULD 支持在 approvals 响应中记录响应方身份（例如 `responded_by_client_id`），以支持审计与“外部编排器弹窗批准”的场景。

#### Scenario: Responder identity is stored
- **WHEN** 客户端调用 `respond_approval` 并提供 `responded_by_client_id`
- **THEN** 该字段被持久化并可在后续 `get_approval` 中读取

### Requirement: Approvals MAY use elicitation when supported by client
如果客户端声明支持 MCP elicitation capability，系统 MAY 在 approvals 创建后主动发起 elicitation 请求以收集用户输入；无论是否启用 elicitation，系统 SHALL 保持 pull 闭环（`list/get/respond_approval`）可用。

#### Scenario: Elicitation is optional and has pull fallback
- **WHEN** 客户端不支持 elicitation 或 elicitation 超时
- **THEN** 客户端仍可通过 `list_approvals/get_approval/respond_approval` 完成审批闭环

