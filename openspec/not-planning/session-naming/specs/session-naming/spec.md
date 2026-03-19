## ADDED Requirements

### Requirement: Session 支持可选的人类可读名称

系统应当（SHALL）为每个 session 存储一个可选的 `name`。

当 session name 缺失时，系统应当（SHALL）在 UI 中展示一个可确定（deterministic）的 fallback label，基于 session ID 派生（例如 UUID prefix），以保证 session 在 UI 中仍可区分。

#### Scenario: Session name 缺失

- **WHEN** session 的 `name = null`
- **THEN** UI 展示从 session ID 派生的 fallback label

### Requirement: 用户可以重命名 session

系统应当（SHALL）允许用户重命名已有 session。

系统应当（SHALL）对输入做 trim，并将空字符串视为清空名称（即 `null`）。

#### Scenario: 重命名 session

- **WHEN** 用户为某个 session 提交新的非空 name
- **THEN** 后续读取该 session 时返回更新后的 name

#### Scenario: 清空 session name

- **WHEN** 用户将 session name 设为空字符串（或 `null`）
- **THEN** 持久化存储的 session name 变为 `null`

### Requirement: 未显式提供 name 时自动命名

当创建 session 时未提供显式 name，系统应当（SHALL）基于创建上下文设置一个“尽力而为”的自动生成 name。

自动命名必须（MUST NOT）覆盖用户显式提供的 name。

#### Scenario: 创建时自动命名

- **WHEN** 创建 session 时未提供显式 name，且后端拥有足够上下文生成 name
- **THEN** 创建出的 session 拥有非空（non-null）的 name
