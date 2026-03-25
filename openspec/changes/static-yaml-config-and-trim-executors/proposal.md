## Why

VK 的配置与“设置”目前分散在多种持久化机制中（JSON 资产配置、DB 持久化的 project/repo 设置、executor profile overrides）。这迫使后端与前端都要维护一套复杂的设置面，增加迁移成本，并在 schema 演进时让变更变得脆弱且难以回滚。

同时，默认构建会携带大量大多数安装并不会使用的 executors，导致编译耗时、二进制体积以及长期维护复杂度都被显著放大。

## What Changes

- **BREAKING**: 引入以 OS 用户配置目录为根（例如 Linux/macOS: `~/.config/vk/`）的 file-first 配置模型，以 YAML 作为唯一事实来源。
- **BREAKING**: 停止将“设置”持久化到数据库。project/repo 配置与 executor/profile 配置迁移到 YAML。数据库仍用于运行时数据（tasks/attempts/logs），但 settings 不再 DB-backed，也不再进行 settings 相关 DB 迁移。
- 增加 `config.yaml`（YAML）作为主配置文件，并在同一配置目录下自动加载 `secret.env`（dotenv）作为 overlay。
- 增加对 YAML 值的环境变量模板解析（例如 `${OPENAI_API_KEY}`），且 **`secret.env` 优先级高于进程/系统环境变量**。
- 后端支持无需重启的配置 reload（文件监听和/或显式 reload endpoint），并在配置无效时具备安全回退策略。
- 为 YAML 配置生成 JSON Schema（用于编辑器校验/hover/补全），并放在同一配置目录下。
- **BREAKING**: 现有 DB 中的 project/repo settings 将被忽略（不再作为配置来源）。如需保留，可使用一次性导出将其写入 `config.yaml`。
- **BREAKING**: 如本次重构需要调整 DB schema，将不提供旧 settings 形态的自动迁移；升级时可能需要用户重置本地 DB。
- secrets 推荐放在 `secret.env` 并通过 `${NAME}` 引用；允许直接写入 `config.yaml`（但不推荐）。
- **BREAKING**: 不再通过 Settings API 写入配置（例如更新 config/profiles/projects）。配置修改以编辑 `config.yaml` 为主，并通过 reload 生效。
- **BREAKING**: 将默认支持的 executors 收敛到两条主力路径：**Claude Code** 与 **Codex**。其他 executors 改为按需启用（Cargo feature）或从默认分发与文档中移除。

## Capabilities

### New Capabilities
- `yaml-user-config`: 将 YAML 配置存放在 OS 用户配置目录中，包含 reload 语义与环境变量模板解析，并体现 `secret.env` 的优先级规则。
- `yaml-config-schema`: 生成并发布 YAML 配置的 JSON Schema，用于 YAML LSP 的校验/hover/补全。
- `executor-minimal-defaults`: 默认构建仅支持 Claude Code + Codex executors；非核心 executors 必须按需启用或从默认集移除。
- `static-project-config`: projects 与 repos 通过 YAML 定义（不再是 DB-backed settings），使配置更简洁且更易迁移/复用。

### Modified Capabilities
- `config-management`: 配置的存储、路径解析与校验规则从 asset-scoped JSON 迁移到 OS-scoped YAML（并增加模板解析与 reload 行为）。
- `fake-agent-executor`: Fake agent 从默认 executor 集中移除，变为非默认/可选（偏 dev/test）。
- `project-management-safety`: 项目创建/编辑从“写 DB 的 settings UI”转为“显式、可校验的文件配置更新”。
- `project-settings-summary`: 项目设置页面反映 file-based 配置（并可能移除 DB-only 元数据，如 created/updated timestamps）。
- `project-git-hooks`: 全局/项目级 git hook skipping 的唯一事实来源迁移到 YAML，并相应调整 UI 交互模型。

## Impact

- Backend config：`crates/config`、`crates/app-runtime`、`crates/server` 的配置相关 routes，以及 `crates/services/src/services/config/versions` 下的配置演进机制。
- Executor runtime：`crates/executors` 的默认 features、默认 profiles，以及任何假设“存在很多 executors”的 UI/profile 选择逻辑。
- Settings + project 管理：`crates/db` 的 project/repo models 与相关 routes/services；`frontend/src/pages/settings/*`（尤其 `ProjectSettings`）以及任何依赖 DB-backed settings 的地方。
- Tooling/docs：YAML schema 的编辑器配置指南、配置目录布局与 secrets 处理；以及可选的 DB project/repo settings 导出工具。

## Reviewer Guide

- 将这次变更视为一次明确的 “config-first” 方向调整：YAML 是唯一事实来源；DB-backed settings 选择移除而非为兼容长期迁移。
- 验收重点是“简单、清晰、可恢复”：更少的 settings 持久化路径、可预测的优先级（`secret.env` > system env）、更小的默认 executor 面。

## Goals

- 让配置可迁移、可审计、且对编辑器友好（YAML + JSON Schema）。
- 消除 settings 相关 DB migrations 的负担，显著降低前端设置页面维护成本。
- 默认仅分发维护中的 executors，以降低编译/运行开销。
- 支持安全的 config reload，加速迭代而无需重启。

## Non-goals

- 保持对现有 DB-backed settings 或旧配置路径的向后兼容。
- 为新的 YAML 配置构建完整的图形化设置编辑器（允许提供最小化的辅助入口，但主工作流是 YAML + LSP）。
- 在本变更中替换 tasks/attempts/logs 的运行时数据库。

## Risks

- File-based config 引入新的故障模式（无效 YAML、缺失 env vars、部分写入等），必须配套清晰诊断与安全回退。
- 移除 DB-backed settings 可能打破依赖现有 Settings UI 与 DB 中元数据/历史的用户预期。
- executor 的 feature-gate/移除默认支持会影响依赖这些 executors 的工作流，需要明确迁移说明与 opt-in 路径。

## Verification

- 单元测试覆盖：
  - 跨平台 config dir 解析
  - YAML 解析 + schema 校验
  - 模板解析优先级（`secret.env` 覆盖 system env）
  - reload 在有效/无效更新下的行为
- 集成测试 / smoke checks 覆盖：
  - 仅存在 YAML config 时启动 VK
  - 运行中 reload 配置
  - 默认构建在 profiles/selection 中只列出 Claude Code + Codex executors
