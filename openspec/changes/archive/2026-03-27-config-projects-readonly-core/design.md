## Context

VK 已完成 file-first YAML 配置重构，但仍残留一些“为了兼容/便利而增加”的复杂性：
- Projects 配置以 YAML 为准，但仍存在同步到 DB 的路径，形成双源与竞态。
- reload 与 public_config/runtime config 的读取/提交路径分散，存在重复 IO 与额外 TOCTOU 窗口。
- watcher 自动 reload 增加了跨平台 notify 差异、竞态与不可预测性。
- schema 生成在启动时写盘属于副作用，不适合成为运行态必经路径。

本设计旨在让配置与 projects 进一步收敛为 **只读 YAML + 内部缓存 + 手动 reload**，同时提升可观测性（dirty）与稳定性。

## Goals / Non-Goals

**Goals:**
- Projects 单一真相：`projects.yaml` / `projects.d/*.yaml` 为准，DB 不作为配置源。
- Projects API 使用稳定 public DTO（去掉伪造时间戳），避免前端排序抖动。
- runtime/public config 的加载从同一磁盘快照产出，减少重复 IO 与 TOCTOU。
- watcher 不自动应用变更：只标记 dirty；手动 reload 成功后清除 dirty。
- schema 生成改为显式 CLI upsert；服务启动不再强制写盘。

**Non-Goals:**
- 不改变 attempts/git/worktree/执行等核心机制。
- 不引入“服务端写 YAML”能力。
- 不在本变更中处理模板解析白名单（单独变更处理）。

## Decisions

1. **Projects DB 同步策略：从“reload 时同步”改为“按需最小化写入（仅满足 FK/历史数据）”**
   - 选择：移除 `AppRuntime::sync_config_projects_to_db()` 的 reload 调用。\n     由于 tasks 等运行时表存在 `project_id -> projects` 的外键约束，DB 中仍需要存在对应的 project 行。\n     因此改为在“创建 task / attempt / milestone / workspace 等会落库的运行时边界”按需 `find_or_create(project_id, name)` 写入最小 project 行（仅 `id + name + timestamps`）。
   - 原因：避免“文件变更触发 DB 写入”的隐式副作用与双源漂移；同时满足 DB 外键约束与历史数据关联需求。
   - 备选：保留 sync，但做全量 reconcile（更新/删除）。该方案更复杂且仍是双源，不优先。

2. **Projects API 不再复用 DB `Project` 结构体**
   - 选择：定义 `ProjectPublic`（ts-rs 导出），从 `Deployment.public_config()` 直接映射生成；不再伪造 `created_at/updated_at`。
   - 原因：稳定、可预测；避免将 DB 模型当作 API contract；减少未来字段演进带来的偶发回归。

3. **一次磁盘快照产出 runtime/public 两份视图**
   - 选择：在 `crates/config` 提供 `try_load_config_pair_from_file(config_path)`（或等价命名），一次读取 `config.yaml`/`projects.yaml`/`projects.d`/`secret.env` 并生成：
     - runtime Config：允许模板展开（现状）
     - public Config：禁止模板展开（现状 public_config），但使用相同 projects merge 语义
   - 原因：减少重复 IO/parse；确保两份视图对应同一代磁盘输入；降低 TOCTOU。

4. **watcher 不自动 reload，只标记 dirty**
   - 选择：保留 watcher 监听配置目录，但回调只设置 `config_dirty=true`（以及更新可观测状态）；不触发 reload。
   - 原因：移除自动 reload 的竞态与跨平台差异，同时仍能提示“文件已变更但未应用”。
   - 备选：完全关闭 watcher（无 dirty 提示）。可作为后续可选开关，但默认保留 dirty 价值更大。

5. **schema 生成改为 CLI upsert（归属到 `vk` CLI，而非 server runtime）**
   - 选择：提供 CLI 命令 `vk config schema upsert` 在 config dir 写入/更新 `config.schema.json` 与 `projects.schema.json`；服务启动路径移除写盘行为。
   - 原因：schema 写盘属于 operator tooling 的一次性操作，不应成为 server runtime 的启动副作用；同时减少权限与平台差异问题。

## Risks / Trade-offs

- [行为变化] 文件保存后不再自动生效，必须手动 reload。
  - 缓解：status 中暴露 dirty；前端显著提示并提供 reload 按钮。
- [隐式依赖] 代码可能依赖 DB project 行存在。
  - 缓解：在 task create / attempt start 路径按需写入最小 project 行；增加回归测试覆盖。
- [兼容性] Projects API DTO 变化导致前端/脚本破坏。
  - 缓解：同步更新前端与 ts-rs types；提供迁移说明。

## Migration Plan

1. 后端先引入 config pair loader + dirty 状态（不改 Projects API），确保 reload/dirty 行为可观测。
2. 调整 projects DB 同步：移除 reload sync，改为按需最小写入（仅满足外键/历史关联），并明确 DB 不作为配置源。
3. 引入 `ProjectPublic` DTO 并切换 `/api/projects` 与相关 WS/补丁流（如适用）。
4. 增加 CLI schema upsert（`vk config schema upsert`），移除启动写盘；更新 UI 指引。
5. 补齐回归测试与文档，发布。
