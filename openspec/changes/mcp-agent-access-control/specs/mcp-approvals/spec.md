# mcp-approvals Specification

## Purpose
为外部编排器（OpenClaw 类）提供纯 MCP 的 approvals 闭环，并确保响应审计字段在“外部编排器弹窗批准”的场景下可用且一致。

## MODIFIED Requirements

### Requirement: Approval responses SHOULD capture responder identity
系统 SHOULD 支持在 approvals 响应中记录响应方身份（例如 `responded_by_client_id`），以支持审计与“外部编排器弹窗批准”的场景。

当客户端未提供 `responded_by_client_id` 时，系统 SHALL 从 MCP peer info 派生默认值并写入持久化存储。

#### Scenario: Responder identity is stored
- **WHEN** 客户端调用 `respond_approval` 并提供 `responded_by_client_id`
- **THEN** 该字段被持久化并可在后续 `get_approval` 中读取

#### Scenario: Responder identity is derived when omitted
- **WHEN** 客户端调用 `respond_approval` 且未提供 `responded_by_client_id`
- **THEN** 系统派生并持久化一个默认 `responded_by_client_id`（例如基于 MCP client name/version）

