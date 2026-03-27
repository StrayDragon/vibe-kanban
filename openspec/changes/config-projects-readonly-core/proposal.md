## Why

当前配置与项目链路仍存在可精简空间：
- Projects 既来自 YAML（file-first），又会被同步到 DB（`sync_config_projects_to_db()`），容易形成“双源 + 漂移”，并带来额外竞态与长期脏数据。
- Projects API 复用 DB `Project` 结构体并伪造时间戳字段（`created_at/updated_at`），导致前端排序/刷新不稳定。
- runtime config 与 public_config 在 reload 时存在重复磁盘读取与重复 parse/normalize/validate，且会扩大 TOCTOU 窗口。
- 自动 watcher reload 带来跨平台差异与竞态复杂度；我们更希望“可预测”的手动 reload，并提供 dirty 提示。
- schema 文件在启动时写盘属于副作用（权限不足/只读目录时会持续告警），更适合改成显式 CLI upsert。

我们希望把 server 收敛为“只读用户配置 + 内部缓存 + 手动触发 reload”的最小核心，减少维护与 bug 面，并让行为更可预测。

## What Changes

- **BREAKING**: Projects API 改为返回显式 `ProjectPublic` DTO（不再复用 DB `Project`，移除伪造 `created_at/updated_at`），前端同步适配。
- **BREAKING**: Projects 的 source of truth 明确为 file-first YAML（`projects.yaml` / `projects.d/*.yaml`），DB 不再作为“第二份配置源”。\n  - DB 仅保留运行态/历史数据所需的最小记录（如需），但其内容不再被当作项目配置来源。\n  - `sync_config_projects_to_db()` 将被移除或降级为“按需写入最小 cache”（以配置为准，规则确定）。
- runtime config 与 public_config 读取合并为“一次磁盘快照产出两份视图”，减少重复 IO/parse，缩小 TOCTOU。
- watcher 不再自动 reload：仅标记 `dirty=true` 并对外可观测；实际应用变更必须由手动 `POST /api/config/reload` 触发（reload 成功后清除 dirty）。
- schema 写盘改为显式 CLI upsert（例如 `server config schema upsert`），服务启动/运行期不再自动写入 schema 文件。

Goals:
- 配置/项目链路“单一真相”：只读 YAML 为准，避免 DB 双源漂移。
- 行为更可预测：改文件不会自动生效，必须手动 reload；但系统能提示 dirty。
- 降低 reload 复杂度：减少重复磁盘读取与 TOCTOU，读写路径更集中。
- 减少运行时副作用：schema 生成改为 CLI 触发。

Non-goals:
- 不删除在线翻译、诊断检查、文件系统浏览、repo 注册/初始化/分支枚举、attempt 执行型 API、scratch、里程碑等能力面。
- 不在 UI/服务端写入 `config.yaml` / `projects.yaml` / `secret.env`。
- 不在本变更中引入新的配置版本兼容层（如需 breaking，则通过清晰报错与迁移指引）。

Risks:
- [BREAKING] Projects API DTO 变更需要同步前端与 types 生成。
  - Mitigation: 同步更新 TS types + hooks + 页面；增加回归测试。
- 若 DB 层逻辑隐式依赖 project 表（外键/Join/默认值），移除 sync 可能引发运行时错误。
  - Mitigation: 明确 DB 仅为运行态；在任务创建/attempt 启动路径做“按需确保最小记录”或移除外键依赖；补齐集成测试。
- 关闭自动 reload 会改变体验（编辑后不自动生效）。
  - Mitigation: dirty 提示清晰；UI/CLI 提供一键 reload（但不自动）。

Verification:
- `cargo test -p config`
- `cargo test -p app-runtime reload`
- `cargo test -p server projects config tasks`
- `pnpm -C frontend run check`

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `static-project-config`: Projects/repo 配置以 `projects.yaml` / `projects.d` 为准，DB 不作为配置源；Projects API 返回稳定的 public DTO。
- `yaml-user-config`: 配置应用为“手动 reload”；watcher 只提供 dirty 提示；runtime/public 视图来自同一磁盘快照以降低 TOCTOU。
- `yaml-config-schema`: schema 生成以 CLI upsert 触发；服务运行期不再强制写盘。

## Impact

- Backend:
  - `crates/app-runtime/src/lib.rs`（load/reload、watcher、dirty flag、schema 写盘移除）
  - `crates/config/src/lib.rs`（新增 load-pair API：runtime+public）
  - `crates/server/src/routes/config.rs`（status 增加 dirty、schema upsert CLI、reload 行为）
  - `crates/server/src/routes/projects.rs`（ProjectPublic DTO）
  - `crates/server/src/routes/tasks.rs`（创建任务时对 project 存在性/最小 DB 记录策略可能调整）
- Frontend:
  - `frontend/src/hooks/projects/useProjects.ts` / Settings Projects 页 / 类型生成

