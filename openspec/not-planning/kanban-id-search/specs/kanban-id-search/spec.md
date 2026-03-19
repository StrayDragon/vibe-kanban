## ADDED Requirements

### Requirement: Tasks 暴露稳定的 number 与 short ID

系统应当（SHALL）暴露：

- `Task.number`：稳定的 numeric identifier
- `Task.short_id`：由 `Task.id` 派生的 deterministic short identifier

这些字段必须（MUST）出现在 kanban UI 使用的 task responses 中。

#### Scenario: task 数据包含标识符

- **WHEN** client 加载某个 project kanban 的 tasks
- **THEN** 每个 task 包含 `number` 与 `short_id`

### Requirement: Kanban search 支持按 number 与 short_id 匹配

kanban search filter 应当（SHALL）按以下规则匹配 tasks：

- `#<number>` 或 `<number>`（精确匹配）
- `short_id` 或 UUID prefix（case-insensitive match）
- title/description（保持现有行为）

#### Scenario: 按 number 搜索

- **WHEN** 用户在 kanban search input 中输入 `#123`（或 `123`）
- **THEN** 只有 `number = 123` 的 task 仍可见

#### Scenario: 按 short_id 搜索

- **WHEN** 用户输入某个 task 的 `short_id`
- **THEN** 该 task 仍可见
