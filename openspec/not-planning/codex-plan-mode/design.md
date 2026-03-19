## 背景

我们通过 `crates/executor-codex` 集成 Codex：启动 Codex app-server，并将其流式事件归一化写入日志存储。前端已经可以渲染来自 Codex `PlanUpdate` 事件的 Todo 条目（被归一化为 `plan` tool entry，action type 为 `TodoManagement`）。

目前缺少的是一个一等（first-class）的 “plan-only” 执行模式，用于保证 agent 不能对 workspace 做任何变更。

## 目标 / 非目标

**目标：**
- 增加可通过 executor profile variant 选择的 Codex plan-only 模式。
- 强制约束：禁止写入、禁止命令执行。
- 当 plan mode 关闭时，保持 Codex 现有的正常执行行为不变。

**非目标：**
- 不重做 executor selection UI。
- 不实现上游完整的 model-selector discovery stack。

## 决策

### 决策：在 Codex executor config 中新增 `plan: bool`

在 Codex executor config struct 中新增 `plan` 布尔值，使其可通过 profiles 启用（例如 CODEX `PLAN`）。

### 决策：在 app-server client 中强制 plan-only 约束

plan-only 必须由 host 强制，而不能只依赖 “instructions”，以避免意外 mutation。

在 plan mode 下：
- 拒绝所有可能导致状态变更的 tool calls（例如 apply_patch、run_command、write_file 类工具）。
- 默认使用 read-only sandbox 配置以增加 defense-in-depth。

### 决策：通过现有 Todo panel 展示计划

我们已经把 Codex `PlanUpdate` 归一化为 TodoManagement 条目。本次不新增 plan UI；只需确保 plan mode 能稳定产出 `PlanUpdate`，并可在相关 UI 位置增加轻量的 “Plan-only” 标识。

## 风险 / 取舍

- **[工具分类]** → 为 plan mode 定义严格 allowlist（只读工具）。未知工具一律拒绝。
- **[用户预期]** → 在 profile selector 中明确标识 plan mode（variant 名 `PLAN`），并可选在 attempt UI 中展示标识。

## 迁移计划

1. 在 Codex config 增加 `plan` 字段，并贯通到 spawn/client 逻辑。
2. 在 `crates/executors/default_profiles.json` 中新增 CODEX `PLAN` variant，并设置 read-only sandbox defaults。
3. 如需要，增加/调整最小化的 UI 标识。
4. 增加回归测试，验证 “plan mode 会拒绝 mutation tools”。

## 开放问题

- plan mode 是否仍需要创建 workspace/worktree？（默认：**是**；可能需要读取 repo 状态，sandbox 仍为 read-only。）
- plan mode 是否允许只读命令执行（例如 `rg`、`cat`）？（默认：初期 **否**；先严格，必要时再扩展。）
