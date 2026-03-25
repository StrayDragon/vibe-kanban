## 1. 配置目录与 YAML 加载基础

- [x] 1.1 增加 VK 配置目录解析（`VK_CONFIG_DIR` override + OS 默认目录），验证：`cargo test -p utils-core` / `pnpm run backend:check`
- [x] 1.2 将系统配置入口从 `asset_dir()/config.json` 切换为 `config.yaml`（缺失/无效回退 defaults），验证：`cargo test -p config`
- [x] 1.3 实现 `secret.env` 自动加载（dotenv），并保证优先级 `secret.env` > process/system env，验证：`cargo test -p config`
- [x] 1.4 实现 YAML 字符串值模板解析（`{{env.NAME}}` / `{{env.NAME:-default}}` / `{{secret.NAME}}`；缺失且无默认值报错），验证：`cargo test -p config`
- [x] 1.5 引入“last-known-good”配置快照与 last error 记录（reload 失败不影响当前运行配置），验证：`cargo test -p config`

## 2. Reload / Status API（不泄露 secrets）

- [x] 2.1 增加配置状态查询（config dir、loaded_at、last error summary；不回显 `secret.env` 值），验证：`pnpm run backend:check` +（`pnpm run dev`）`curl -s http://localhost:<BACKEND_PORT>/api/config/status`
- [x] 2.2 增加显式 reload（`POST /api/config/reload`）并接入快照切换语义，验证：`pnpm run backend:check` +（`pnpm run dev`）修改 `config.yaml` 后执行 `curl -s -X POST http://localhost:<BACKEND_PORT>/api/config/reload`
- [ ] 2.3（可选）增加 `notify` 文件监听（去抖）自动触发 reload，验证：`pnpm run dev` 后编辑 `config.yaml` 观察日志与 `/api/config/status`
- [x] 2.4 禁用/移除 settings 写入类 endpoints（例如 `PUT /api/config`、`PUT /api/profiles`、`POST/PUT/DELETE /api/projects`），并在响应中提示“编辑 `config.yaml` + reload”，验证：对上述 endpoints 发起请求返回 `405`（或等价错误）

## 3. YAML JSON Schema（YAML LSP）

- [x] 3.1 从 Rust config types 生成 `config.schema.json` 并写入配置目录（原子写入），验证：启动后文件存在 + `pnpm run backend:check`
- [x] 3.2 为关键字段补充 schema 描述（包含 `project git_no_verify` override precedence），验证：生成的 `config.schema.json` 中包含相关 `description`
- [x] 3.3 更新文档/提示，支持 `config.yaml` 使用 `# yaml-language-server: $schema=./config.schema.json`，验证：`rg \"yaml-language-server\" -n`

## 4. Executors 收敛（默认仅 Claude Code + Codex）

- [x] 4.1 将 `crates/executors` 默认 features 收敛到 `claude` + `codex`，验证：`cargo check -p executors`
- [x] 4.2 更新默认 profiles（只包含受支持 executors）与配置校验（引用不可用 executor 给出清晰错误），验证：`cargo test -p executors`
- [x] 4.3 将 profiles/overrides 的 source-of-truth 迁移到 `config.yaml` 并移除 `profiles.json` 读写路径（保留 `GET /api/profiles` 用于 UI），验证：`pnpm run backend:check` +（`pnpm run dev`）`curl -s http://localhost:<BACKEND_PORT>/api/profiles`
- [x] 4.4 将 Fake agent executor 变为非默认/可选（feature gate），验证：`cargo check -p executors`（默认）与 `cargo check -p executors --features fake-agent`

## 5. Projects / Repos 静态化（YAML）与 DB-backed settings 移除

- [x] 5.1 定义 projects/repos 的 YAML 结构（稳定 id、repo paths、hooks/policy 等）并纳入 schema，验证：`cargo test -p config` + schema 生成
- [ ] 5.2 后端以 YAML 作为 projects/repos 的唯一事实来源（停止写入 DB settings），验证：`pnpm run backend:check` +（`pnpm run dev`）`curl -s http://localhost:<BACKEND_PORT>/api/projects` 返回与 `config.yaml` 一致
- [ ] 5.3 对“orphaned runtime history”（DB 中引用了不存在的 project id）提供 `Unknown project` 占位展示，验证：前端任务列表/详情不崩溃 + 基本可追溯
- [ ] 5.4 新增/调整最小化 Settings UI：展示 config dir、打开文件/目录、reload、校验错误（last error + last-known-good），验证：`pnpm run check` + 手动 UI smoke
- [ ] 5.5（可选）提供“项目配置片段生成器”（生成 YAML snippet + Copy），引导用户粘贴到 `config.yaml` 并 reload，验证：手动 UI smoke
- [ ] 5.6（可选）提供一次性导出：将现有 DB project/repo settings 导出为 YAML（不导出 secrets），验证：`cargo run --bin <export-bin> -- --out <path>` 生成文件可被 loader 读取
- [ ] 5.7 如共享类型变更，更新 Rust DTO 并运行 `pnpm run generate-types`，验证：`pnpm run check`

## 6. 清理与回归验证

- [ ] 6.1 移除旧的 `config.json` / `profiles.json` 读写路径与相关设置迁移逻辑，验证：`cargo test --workspace`
- [ ] 6.2 端到端手动验证：`pnpm run dev`，覆盖启动、reload、executor 选择、project 列表与基础任务流
- [ ] 6.3 如 DB schema 发生 breaking 变化，增加启动期检测与可操作的报错（提示导出 settings + 重置 DB），验证：使用旧 DB 启动时返回明确错误信息/文档指引
