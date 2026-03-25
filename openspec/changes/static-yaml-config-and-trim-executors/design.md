## Context

VK 当前存在多条“settings”持久化路径：

- **System config**：JSON 存在 asset dir 下（见 `utils_assets::config_path()` → `asset_dir()/config.json`）。
- **Executor profiles**：JSON 存在 asset dir 下（`profiles.json`），默认 profiles 内嵌在 `executors` workspace。
- **Project / repo settings**（scripts、scheduler policy、hooks、allow-lists 等）：DB-backed（`crates/db/src/models/project.rs`、`project_repo`、`repo`，以及前端 `frontend/src/pages/settings/ProjectSettings` 的设置交互）。

这会显著抬高配置变更成本：
- 前端必须维护与后端 DB schema 强绑定的表单与交互
- 后端必须维护 migrations 与“settings”相关 DTO 演进
- 出问题时往往需要迁移/修复步骤，而不是简单改文件即可恢复

此外，默认构建会启用许多可选 executor providers（`crates/executors/Cargo.toml` 的 default features），即使大多数安装只使用 Claude Code 与 Codex，也会导致编译时间、二进制体积和需要长期维护的表面积显著增加。

本变更将 VK 转向以 OS 用户配置目录为根的 **file-first**、编辑器友好的 YAML 配置模型，并收敛默认 executor 集。

## Goals / Non-Goals

**Goals:**
- 将配置的唯一事实来源存放为 OS 用户配置目录下的 **YAML**（例如 Linux/macOS: `~/.config/vk/`）。
- 支持确定性的环境变量模板解析，且优先级为：`secret.env` > process/system env。
- 提供安全的 **reload** 语义（无需重启），并具备清晰诊断与“last known good”回退。
- 生成并发布用于 YAML LSP 的 JSON Schema（校验/hover/补全）。
- 将默认 executor 面收敛到 **Claude Code + Codex**。
- 移除 projects/repos/profiles/config 的 DB-backed “settings” 流程（DB 仍用于运行时 attempt/task 数据）。

**Non-Goals:**
- 保留对 DB-backed settings 或旧 JSON asset config 路径的向后兼容。
- 实现一个完整的图形化配置编辑器；主工作流是 YAML + LSP。
- 引入运行时动态插件加载（当前用 Cargo features/optional crates 足够）。

## Decisions

### 1) Config directory is OS user config dir (with an explicit override)

**Decision:** 通过 OS 约定解析 config root，并为高级用户与测试提供 override：
- Linux/macOS: `~/.config/vk/`
- Windows: `%APPDATA%/vk/` (exact path via `directories`/`dirs`)
- Override: `VK_CONFIG_DIR=/path/to/dir`

**Why:** 这更符合安装版应用的用户预期（配置不在 repo 内），也符合“secrets 应位于非版本控制目录”的诉求。

**Alternatives considered:**
- Repo-root `config/`：更利于版本控制，但对安装版应用与 secrets 管理不友好。
- OS data dir：更适合大体积运行时产物，不适合作为用户可编辑配置的主要入口。

### 2) One canonical YAML file + small sidecar files

**Decision:** 使用单一主 YAML 文件 + 少量约定的 sidecar 文件：
- `config.yaml` (canonical config)
- `secret.env` (dotenv-format secret overlay; optional)
- `config.schema.json` (generated; optional but recommended)

**Why:** 单文件让心智模型最简单，同时又允许通过 `secret.env` 将 secrets 分离出来。

**Config 结构草案（示例，非最终字段名）：**

`config.yaml`（示例）
```yaml
config_version: v11

access_control:
  mode: TOKEN
  token: ${VK_ACCESS_TOKEN}

github:
  pat: ${GITHUB_PAT}

executors:
  default_profile: CLAUDE_CODE/DEFAULT
  profiles:
    - id: CLAUDE_CODE/DEFAULT
      executor: CLAUDE_CODE
    - id: CODEX/DEFAULT
      executor: CODEX

projects:
  - id: 9f3d8e22-7cbe-4f0c-a2c8-1c9f2b8b7c61
    name: my-project
    repos:
      - path: /abs/path/to/repo
    git_no_verify_override: null # null=inherit, true/false=override
```

`secret.env`（示例）
```env
VK_ACCESS_TOKEN=...
GITHUB_PAT=...
OPENAI_API_KEY=...
```

说明：
- schema 以 Rust types 为准；示例只用于传达边界与工作流。
- secrets 推荐通过 `${NAME}` 从 `secret.env` 注入；不建议把 token 直接写进 `config.yaml`。

**Alternatives considered:**
- 多个 YAML 文件（`projects.yaml`、`profiles.yaml`…）：更模块化，但会增加协调成本与“局部 reload”复杂度。
- YAML includes/anchors：能力强但会显著增加解析复杂度，并让 schema 校验更难落地。

### 3) Template resolution happens after YAML parsing and before typed deserialization

**Decision:** 先将 YAML 解析为未类型化的 `serde_yaml::Value`，遍历并仅对标量字符串做模板替换，再反序列化为 Rust typed structs。

**Template rules (initial):**
- 支持的模式：`${NAME}` 与 `${NAME:-default}`
- 解析顺序：优先 `secret.env` 映射，其次 `std::env`
- 缺失且无默认值的变量视为 **错误**（config invalid）
- 仅对 YAML **字符串值**（不包含 key）做替换，避免意外的类型转换与语义漂移

**Why:** 该方式避免了 YAML 结构层面的注入风险，并让模板解析更可预测、可调试。

**Alternatives considered:**
- Pre-parse string substitution: simpler but unsafe (can break YAML structure).
- Full template engines (Handlebars/Tera): too much power and unclear security story.

### 4) Config reload uses “last known good” snapshots + explicit status reporting

**Decision:**
- 维护一份内存中的 `ConfigSnapshot`，由 `Arc<RwLock<...>>`（或等价机制）保护。
- reload 时：
  - parse + resolve + validate 得到新的 snapshot
  - 成功：原子交换 snapshot 并更新 “loaded_at”
  - 失败：保留旧 snapshot，记录错误诊断信息，并通过 status endpoint 暴露

**Triggers:**
- `POST /api/config/reload`（显式触发）
- 可选：使用 `notify` 对 `config.yaml` 与 `secret.env` 做去抖文件监听

**Why:** 运维/开发可以快速迭代配置，同时错误配置不会直接“炸掉”正在运行的服务进程。

**Alternatives considered:**
- 无效配置直接 hard-fail：更“安全”，但会显著降低迭代速度与恢复能力。
- 仅靠 watcher 自动 reload：调试更难，且会被编辑器的多次写入模式放大噪声。

### 5) JSON Schema generation is part of the runtime and emits a stable file

**Decision:** 从 Rust config types 生成 `config.schema.json`（通过 `schemars`/现有 schema 生成代码），并在启动时和/或按需写入 config dir。

Editor guidance:
- 建议在 `config.yaml` 中添加如下 header：
  - `# yaml-language-server: $schema=./config.schema.json`

**Why:** YAML + LSP 成为主要 UX，可显著减少定制 settings UI 的维护需求。

### 6) Versioning: single evolving schema, explicit breaking bump, no auto-migration

**Decision:** 在 `config.yaml` 中保留 `config_version`（或 `version`）字段，但将配置视为 **单一最新 schema**：
- 增量（additive）变更：通过 defaults 处理
- 破坏性（breaking）变更：显式 bump 版本并要求人工更新（不做兼容 shim）

**Implementation constraint:** 任何配置语义的变更都需要在 `crates/services/src/services/config/versions` 中体现（即使“迁移”只是 hard break），以保证代码库中存在单一权威的配置演进入口。

### 7) Executors: default build supports only Claude Code + Codex

**Decision:** 将 `crates/executors` 的 default features 收敛为：
- `claude`
- `codex`

其他 executors（包括 Fake agent）改为通过 Cargo features 按需启用；未显式启用时视为不支持并不对外提供选择入口。

**Why:** 这能降低编译成本与维护面，同时为 dev/test 保留“逃生舱”（按需启用）。

**Alternatives considered:**
- 默认启用全部 executors：保留广度，但违背“极简化”的核心目标。
- 运行时插件加载：更灵活，但复杂度显著增加（ABI、discoverability、打包分发）。

### 8) DB-backed settings are removed; optional export is provided

**Decision:** 将 file-based config 作为 projects/repos/profiles/settings 的唯一事实来源。为希望在 hard cut 前保留现有 DB 配置的用户提供可选导出工具。

**Export approach (optional):**
- 提供 CLI 命令（或 admin endpoint）将 DB project/repo settings 导出为 `config.yaml` 等价结构。
- 导出后，操作者应使用 YAML config 重启，并接受 DB-backed settings 已被废弃/忽略。

**Why:** 用户明确偏好“一步到位”，且接受必要时重建/清空 DB。

**Migration note:** 本变更不承诺迁移/保留现有 DB 中的“settings”与其历史形态；如实现过程中需要调整 DB schema（例如移除 project/repo settings 表/列），建议提供清晰的启动期诊断并要求用户重置本地 DB（或先导出 settings 到 `config.yaml`）。

### 9) 配置变更不再通过 Settings API 写入

**Decision:** 不再提供“通过 HTTP API 修改 config/profiles/projects 并落盘”的 settings 工作流；配置修改以编辑 `config.yaml` 为主，并通过 reload 生效。UI 仅提供：
- config dir/文件的定位与打开
- reload 触发
- 校验错误与 last-known-good 状态展示

**Why:** 这显著降低前端 settings 页面与后端写入/迁移逻辑的复杂度，并与“YAML + LSP”作为主工作流一致。

## Risks / Trade-offs

- **无效 YAML / 缺失 env vars** → 通过 last-known-good snapshots、显式 status endpoint、清晰错误日志来缓解。
- **通过 API/logs 泄露 secrets** → 对 secret 字段做 redact，并且绝不回显 `secret.env` 内容；对必需变量仅展示“present/missing”。
- **文件写入安全（schema 生成 / 可选导出）** → 使用原子写入（临时文件 + rename）并检查目录权限。
- **默认移除 executors 的破坏性影响** → 通过明确文档、build flags，以及在 `/api/info` 中暴露“supported executor set”来缓解。

## Migration Plan

1. 增加 config-dir 解析 + `VK_CONFIG_DIR` override，并实现 YAML load/validate/template resolve。
2. 实现 `secret.env` 加载与优先级规则。
3. 实现 schema 生成（`config.schema.json`）与编辑器引导。
4. 实现 reload endpoint + 可选 watcher + status endpoint（last load time + last error）。
5. 收敛 executor 默认 features；更新默认 profiles 与任何依赖“很多 executors 存在”的选择 UX。
6. 将 projects/repos/profiles 的 settings 读写从 DB-backed 替换为 YAML-backed。
7.（可选）提供将现有 DB project/repo settings 导出为 YAML 的一次性工具。
8. 更新文档并移除/退役不再适用的 legacy settings UI 路径。

Rollback strategy:
- 若 YAML config 无效，服务继续使用 last-known-good snapshot 运行。
- 若必须回滚部署，操作者可 pin 到旧版本 VK 并保留旧 DB/settings；本变更不尝试做 forward/back migrations。

## Open Questions

- 长期来看是否需要 “includes” 或多文件组合配置，还是单一 `config.yaml` 就足够？
- 当 reload 改变 executor 可用性或 policy allow-lists 时，是否需要显式确认？
- 从 DB-backed settings 切到 YAML 后，“被删除”的 projects/repos 要如何表示（忽略，或作为 orphaned runtime history 展示）？
