## Context

当前 VK 的 projects / repos 配置以 YAML 文件为事实来源（`projects.yaml` 与 `projects.d/*.yaml`）。为了避免“自动改写用户配置文件”带来的格式化/注释丢失、以及潜在安全风险，相关 settings 写入 API 目前被禁用，UI 也主要以“只读 + 指引（路径、复制片段、reload）”为主。

但这也带来了两个实际痛点：

1) 日常“新增仓库”需要手动编辑 YAML，成本高且容易出错（尤其在配置静态化改动之后）。
2) Kanban 多处“+”入口存在二级菜单（新建任务 / 新建任务组），交互不一致且冗余。

技术约束与现状：

- `crates/config` 在加载 projects 时，会把 `projects.yaml` 与 `projects.d/*.yaml` 的 `projects:` 列表简单拼接后再校验；同一 `project.id` 不能出现在多个文件中（否则会被校验为重复 id）。
- 因此，“通过新建一个 `projects.d` 片段来给既有项目追加 repo”在当前语义下不可行（会产生重复 project id）。

## Goals / Non-Goals

**Goals:**

- 在不直接改写用户现有 `projects.yaml` / `projects.d/*.yaml` 的前提下，实现“通过路径新增仓库，并出现在项目仓库列表中”。
- 新增仓库流程具备可解释的校验与错误提示，并尽量做到幂等（重复添加同一路径不造成配置污染）。
- Kanban 创建入口统一：点击“+”直接打开创建弹窗，不再出现二级菜单，同时保留创建任务组能力。

**Non-Goals:**

- 不实现“完整配置编辑器”或“任意字段可写”的 settings 体验。
- 不引入自动写入 `config.yaml` / `projects.yaml` / `secret.env` 的通用能力。
- 不在本次变更中重做 projects.d 的合并语义（仍保持当前“拼接 + 唯一校验”的策略）。

## Decisions

### 1) 通过 VK 管理的 overlay YAML 实现“追加 repo”

为实现“追加”而不触碰用户原始文件，本变更引入一个新的可选输入源（overlay YAML），由 VK 自己维护，专门承载“对某个 project 追加 repos”的最小增量数据。

- **文件位置**：位于 config dir（与 `projects.yaml` 同级），例如 `projects.ui.yaml`（最终命名以实现为准）。
- **文件语义**：仅允许“追加 repos”，不支持删除/重排/修改既有 repos，避免覆盖用户手写配置。
- **加载顺序**：
  1. 先按现有逻辑加载 `projects.yaml` + `projects.d/*.yaml`
  2. 再读取 overlay YAML，并对匹配的 `project_id` 执行“追加 repo（去重）”
  3. 最后走既有的 normalize + validate 流程，确保最终运行时配置仍满足 schema（例如 repo path 必须是绝对路径、repo path 在同一项目内唯一等）

这样做的取舍：

- ✅ 不会改写用户已有 YAML（保留注释/格式/组织方式）
- ✅ overlay 文件可以随时删除回滚（恢复为纯静态配置）
- ❌ 需要在 `crates/config` 增加一个额外输入源与合并逻辑

### 2) API 侧：提供“按路径添加仓库”的显式操作，并触发 reload

新增一个专用 API（或复用/替换已禁用的 `POST /api/projects/{id}/repositories` 写入路径）来执行：

1. 校验用户输入路径：
   - 存在且为目录
   - 解析为绝对路径（支持 `~` 展开；可选：对 Git 仓库取 `git rev-parse --show-toplevel` 作为最终路径）
2. 读取当前配置，确认 `project_id` 存在
3. 更新 overlay YAML：把 repo 追加到对应 project 的 repo 列表（幂等去重）
4. 触发 `deployment.reload_user_config()`，让新增仓库立即可见

安全边界：

- 写入仅发生在 VK config dir 内的 overlay 文件中；拒绝写任意路径。
- overlay 中仅写入与 repo 追加相关的最小字段（例如 path、display_name），不允许写入脚本字段与 secret 引用。

### 3) UI 侧：Kanban 项目下拉新增“添加仓库…”弹窗

在 Kanban 的项目下拉中添加一个入口：

- 点击后打开一个简洁弹窗：输入 `path`（可选：display_name），提交后调用上述 API
- 成功后刷新项目仓库列表（或依赖 projects stream / reload 触发刷新）
- 失败时展示明确错误原因（路径无效、非绝对路径、项目不存在、重复路径等）

### 4) Kanban “+” 入口：移除二级菜单，直接打开统一创建弹窗

把现有“+”触发的下拉菜单改为：

- 点击即打开创建弹窗（默认创建任务）
- 弹窗内提供轻量的类型切换（任务 / 任务组），以保留任务组创建能力但不再占用入口二级菜单

## Risks / Trade-offs

- **[overlay 文件写入失败]**（权限/只读目录）→ UI 提示“无法写入 overlay”，并回退到“复制 YAML 片段 + 打开 config dir”的手动指引（可作为后续增强）。
- **[Git 探测不可用]**（系统无 git 或命令失败）→ 仍允许按目录路径添加，但不做 repo root 纠正；在响应中返回探测状态供 UI 提示。
- **[配置 reload 失败]** → 保持 last-known-good 配置；API 返回失败并提示查看 `/api/config/status` 的 last_error。
- **[overlay 漂移]**（用户手动删除/改动 base 项目）→ reload 时 overlay 中引用的 project_id 找不到则报错并提示清理 overlay 相关条目。

## Migration Plan

该变更为增量能力：

- 默认不存在 overlay 文件；不影响现有用户。
- 通过 UI 添加 repo 后才会创建 overlay 文件并写入最小内容。
- 回滚方式：删除 overlay 文件（例如 `projects.ui.yaml`）并 reload。

## Open Questions

- overlay 文件最终命名与是否需要对应的 schema 生成（例如 `projects.ui.schema.json`）。
- “创建弹窗”中任务/任务组的默认与是否需要记忆上次选择。
- Git 探测范围：仅识别 repo root，还是也读取 remote URL / default branch（若读取则需明确不写入配置、仅用于 UI 展示）。

