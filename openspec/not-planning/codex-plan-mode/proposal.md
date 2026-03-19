## 为什么

用户经常希望在允许 agent 执行代码变更前，先看到一份高质量、可执行的计划。“仅计划（plan-only）”模式可以让这个流程更安全、更高效：

- agent 只输出可执行计划（steps/todos），不运行命令、不修改文件
- UI 可以先展示计划供用户审阅，再由用户决定是否启动正常的执行型 run

上游 Vibe Kanban 已引入 Codex plan mode，我们也应在 Codex executor 集成中提供同等能力。

## 变更内容

- 在 Codex executor 配置（`crates/executor-codex`）中新增 `plan` 开关，用于启动 Codex 的 plan-only 模式。
- 在 `crates/executors/default_profiles.json` 中新增 CODEX 的 `PLAN` profile variant，使用户可在现有 profile selector UI 中直接选择。
- 强制执行 plan-only 约束（以“严格”为默认策略）：
  - 禁止命令执行
  - 禁止任何文件系统写入（包括 patch application）
  - 默认使用 read-only sandbox
  - 对未知/未分类工具调用采取拒绝策略（防止遗漏导致可变更）
- 确保计划输出能通过现有 Todo/Plan UI 展示（Codex 的 `PlanUpdate` 事件已经会被归一化为 TodoManagement 条目）。

## 能力

### 新增能力

- `codex-plan-mode`：Codex 支持“仅计划”运行，输出结构化计划并在不修改 workspace 的情况下退出。

### 变更的能力

<!-- 无 -->

## 影响范围

- `crates/executor-codex`：配置 schema + client 侧对 plan mode 的处理与强制约束。
- `crates/executors/default_profiles.json`：新增 CODEX `PLAN` variant。
- UI：可选的轻量标识（例如在 attempt 相关位置显示 “Plan-only”），范围保持最小。

## 目标 / 非目标

**目标：**
- 为 Codex 提供安全的 plan-only 运行模式（保证不会修改 workspace）。
- 复用现有 Todo 面板展示结构化计划，无需新增复杂 UI。

**非目标：**
- 不实现上游那种完整的“模型发现/选择器”体系。
- 不改动非 Codex executors 的行为。

## 风险

- Codex 仍可能尝试发起工具调用 → 必须在 host 侧强制拒绝可变更类请求，而不能只依赖“提示词约束”。
- “什么算 mutation”存在边界争议 → 默认采取严格策略：拒绝 `apply_patch`、文件变更、命令执行，以及任何不在 allowlist 的工具。

## 验证方式

- 使用 CODEX `PLAN` 启动一个 attempt，确认：
  - 没有文件被修改
  - 没有命令执行
  - Todo 面板出现计划（PlanUpdate）
- 运行 `pnpm run backend:check` + `pnpm run check` + `cargo test --workspace`。
