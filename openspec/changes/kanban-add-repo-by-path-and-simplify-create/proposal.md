## Why

最近项目/仓库配置做了较多“静态化（YAML 文件优先）”调整后，新增仓库通常需要手动编辑 `projects.yaml` / `projects.d/*.yaml`。这对日常使用不友好：路径与字段结构容易写错、需要记忆 schema、并且改完还需要手动 reload 才能生效。

同时，Kanban 界面中多个“+”入口会弹出“新建任务 / 新建任务组”的二级菜单，交互不一致且增加了额外点击成本。

## What Changes

- Kanban 视图的项目下拉中新增“添加仓库…”入口：用户提供一个本地路径后，系统自动判断并生成仓库配置（例如识别 Git 仓库根目录、推断仓库显示名等），并写入一个由 VK 管理的 overlay YAML 文件来“追加”到当前项目配置中（不修改既有 `projects.yaml` / `projects.d/*.yaml` / `config.yaml`）。
- 写入完成后提供清晰的成功/失败反馈，并引导或触发一次配置 reload，使新增仓库立即出现在项目仓库列表中。
- Kanban 界面中现有的“+”入口移除二级菜单：点击后直接打开统一的创建弹窗；在弹窗内以一致的方式选择创建“任务”或“任务组”（默认创建任务）。

## Capabilities

### New Capabilities

- `repo-add-by-path`: 在 UI 中通过输入本地路径追加仓库配置（生成 `projects.d` 片段），并在必要时触发/引导配置 reload。
- `kanban-create-one-step`: Kanban 创建入口统一为“一次点击即打开创建弹窗”，移除“新建任务/新建任务组”的二级菜单。

### Modified Capabilities

<!-- 本次变更不修改已有 capability 的 REQUIREMENTS；仅新增能力与 UI 行为。 -->

## Impact

- Frontend: Kanban 项目下拉交互与创建入口交互；创建弹窗组件的统一化调整。
- Backend/Services: 路径校验与 Git 探测；生成并写入 VK 管理的 overlay YAML；（可选）触发配置 reload；刷新项目配置视图。
- Config: 新增一个 overlay YAML 输入源，不改动现有 `projects.yaml` / `projects.d` 文件与版本。

## Goals

- 让“新增仓库”回到可在 Kanban 内完成的流程，减少手动编辑 YAML 的频率与出错率。
- 保持 YAML 文件为配置事实来源（source of truth），并通过最小、可回滚的 `projects.d` 片段追加配置。
- 统一 Kanban 创建交互，减少点击路径与心智负担。

## Non-goals

- 不提供一个“完整配置编辑器”来直接编辑/覆盖 `projects.yaml` 或 `config.yaml`。
- 不引入写入 `secret.env` 的能力，也不在 API/日志中暴露任何 secret 值。
- 不改变现有配置版本语义（除非实现阶段发现必须做破坏性变更并另起版本）。

## Risks

- 路径与权限：用户提供的路径可能不存在/不可读，或 VK 进程无权限读取/写入 config dir。
- Git 探测稳定性：目标目录可能不是 Git 仓库、或系统缺少 `git`，需要明确降级行为与错误信息。
- 配置冲突：重复追加同一路径/同名仓库可能导致配置重复；需要确定去重/提示策略。

## Verification

- 在 Kanban 项目下拉中通过路径新增仓库后，新仓库出现在项目仓库列表中；重启后仍存在（因为已写入 overlay YAML）。
- 写入失败（无权限/路径无效/重复等）时 UI 提示可理解且不会破坏现有配置。
- Kanban 各处“+”入口点击后直接打开创建弹窗，不再出现二级菜单；仍能创建任务与任务组。
