# archived-kanbans Specification (Incremental)

## MODIFIED Requirements (MCP)

### Requirement: MCP 提供 archived-kanbans 工具集

系统 SHALL 在 MCP server 中提供 ArchivedKanban 相关工具集，用于：
- 列出 Project 下的 ArchivedKanbans：`list_archived_kanbans(project_id)`
- 触发归档（创建 archive 并移动 tasks）：`archive_project_kanban(project_id, statuses, title?)`
- 触发还原（批量还原）：`restore_archived_kanban(archive_id, restore_all?, statuses?)`

系统 MUST NOT 通过 MCP 暴露删除归档能力（即：不提供 `delete_archived_kanban` tool）。删除仍可通过 HTTP/UI 执行。

这些 tools MUST 提供 `structuredContent`，并在 `tools/list` 中发布准确的 `outputSchema`。

对于可能耗时的批量写操作，系统 MUST 在 `tools/list` 中声明其为 task-capable：
- `archive_project_kanban` MUST 声明 `execution.taskSupport=optional`
- `restore_archived_kanban` MUST 声明 `execution.taskSupport=optional`

#### Scenario: archived-kanban tools are discoverable and task-capable
- **WHEN** a client requests the MCP tool list (`tools/list`)
- **THEN** the list includes `list_archived_kanbans`, `archive_project_kanban`, and `restore_archived_kanban`
- **AND** `archive_project_kanban` and `restore_archived_kanban` include `execution.taskSupport=optional`
- **AND** the list does NOT include `delete_archived_kanban`

#### Scenario: archived-kanban tools return structured results
- **WHEN** a client calls any archived-kanban MCP tool
- **THEN** the tool result includes non-empty `structuredContent` and its semantics match the tool documentation

## REMOVED Requirements (MCP)

### Requirement: MCP exposes delete_archived_kanban
**Reason**: `delete_archived_kanban` is irreversible and too risky to expose to “other agents” by default.
**Migration**: Use HTTP `DELETE /api/archived-kanbans/:id` (or the UI) for explicit deletion.

#### Scenario: delete tool is not available via MCP
- **WHEN** a client attempts to call `delete_archived_kanban` via MCP
- **THEN** the request fails because the tool is not registered/exposed by the MCP server
