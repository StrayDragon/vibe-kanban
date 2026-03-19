## ADDED Requirements

### Requirement: Codex 支持 plan-only mode

系统应当（SHALL）提供可通过 executor profile variant 选择的 Codex plan-only mode。

#### Scenario: 选择 Codex plan-only mode

- **WHEN** 用户为某个 attempt 选择 CODEX `PLAN` profile variant
- **THEN** 该 attempt 以 plan-only mode 运行 Codex

### Requirement: plan-only mode 不应修改 workspace

在 plan-only mode 下，系统应当（SHALL）阻止对 workspace 的 mutation，包括：

- 不进行任何文件系统写入（包括 patch application）
- 不执行任何可能改变状态的命令

#### Scenario: mutation tool calls 被拒绝

- **WHEN** Codex 在 plan-only mode 下尝试发起 mutation tool call
- **THEN** 系统拒绝该请求，且不发生任何 mutation

### Requirement: plan-only mode 输出结构化 plan

在 plan-only mode 下，Codex 应当（SHALL）输出结构化 plan，并在 UI 中以 Todo/Plan 条目展示。

#### Scenario: plan 出现在 UI 中

- **WHEN** Codex 完成一次 plan-only run
- **THEN** UI 在 Todo panel 中展示最新的 plan steps
