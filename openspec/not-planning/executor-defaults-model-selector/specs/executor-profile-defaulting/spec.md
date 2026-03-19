## ADDED Requirements

### Requirement: DEFAULT variant 以 `null` 表示

系统应当（SHALL）将 executor profile 的 DEFAULT configuration 表示为 `variant = null`（缺失）。

系统应当（SHALL）在持久化边界将 `variant = "DEFAULT"`（case-insensitive，trimmed）归一化为 `variant = null`，确保存储表示是 canonical。

#### Scenario: 加载 legacy config（DEFAULT variant string）

- **WHEN** config file 包含 `executor_profile.variant = "DEFAULT"`
- **THEN** normalized in-memory config 的 `executor_profile.variant = null`

#### Scenario: 保存 config 时持久化 canonical 表示

- **WHEN** 用户保存的 config 中所选 configuration 为 DEFAULT
- **THEN** 写入的 config 存储 `executor_profile.variant = null`

### Requirement: 新 attempt 的默认 profile 解析一致

创建新 attempt 时，UI 应当（SHALL）按以下优先级顺序解析默认 `executor_profile_id`：

1. milestone node override（locked）
2. 用户在对话框中的选择
3. 该 task/attempt 的 last used coding-agent `executor_profile_id`
4. 用户系统默认 executor profile

#### Scenario: milestone node profile 被锁定

- **WHEN** task 为 milestone node 且设置了 `executor_profile_id`
- **THEN** 对话框选择该 profile 并禁用编辑

#### Scenario: 存在 last used profile 时应优先使用

- **WHEN** 存在一个 previous attempt，且可确定其 last used coding-agent `executor_profile_id`
- **THEN** 对话框应预选中该 profile（包含 variant）

#### Scenario: fallback 到系统默认

- **WHEN** 无法获得 last used coding-agent profile
- **THEN** 对话框应预选中用户系统 `executor_profile`

### Requirement: attempt summaries 暴露 last used coding-agent profile

当返回 attempt creation UI 使用的 attempt summaries 时，系统应当（SHALL）为每个 attempt 提供 last used coding-agent `executor_profile_id`（executor + variant）。

#### Scenario: attempt list 包含 last used profile

- **WHEN** client 获取带 session metadata 的 task attempts 列表
- **THEN** 每个 attempt 包含 last used coding-agent `executor_profile_id`；若 coding-agent process 尚未运行则为 `null`
