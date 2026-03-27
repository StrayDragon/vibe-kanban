## 1. Config 加载与两视图（runtime/public）收敛

- [x] 1.1 在 `crates/config` 增加“一次磁盘快照产出两份视图”的入口（例如 `try_load_config_pair_from_file()`）：runtime 允许模板展开（维持现状语义）；public 禁止模板展开，但 projects merge 语义一致
- [x] 1.2 `crates/app-runtime/src/lib.rs` 使用新入口改造 cold start 与 reload：避免重复读盘/parse；保证 runtime/public/status 同代提交
- [x] 1.3 增加回归测试：runtime/public 同代一致性（同一次读取下 projects 覆盖语义一致；public 不展开模板）

Verification:
- `cargo test -p config`
- `cargo test -p app-runtime reload`

## 2. Dirty 提示 + 手动 reload（禁用自动应用）

- [x] 2.1 调整 watcher：监听配置目录变更时不触发 reload，仅设置 `dirty=true`（并写入可观测状态）
- [x] 2.2 `POST /api/config/reload` 成功后清除 dirty；失败时保持 dirty 并记录错误
- [x] 2.3 更新前端：在 Settings/相关位置展示 dirty 状态与明确提示（“已修改但未应用，点击 reload”）
- [x] 2.4 增加回归测试：文件变更触发 dirty；未 reload 前 active config 不变；reload 后 dirty 清除

Verification:
- `cargo test -p app-runtime`
- `cargo test -p server config`
- `pnpm -C frontend run check`

## 3. Schema 生成改为 CLI upsert（移除启动写盘副作用）

- [x] 3.1 新增 CLI：`vk config schema upsert`，写入/更新 `config.schema.json` 与 `projects.schema.json`
- [x] 3.2 移除服务启动路径中对 schema 的写盘（避免只读目录持续 warn）
- [x] 3.3 更新文案/指引：告诉用户用 CLI 生成 schema（并提供可复制命令）
- [x] 3.4 增加回归测试：CLI upsert 生成文件且不包含 secrets；只读 config dir 下 server 仍可启动（schema 生成失败不阻塞）

Verification:
- `cargo test -p server`

## 4. Projects 单一真相 + Public DTO（去 DB 双源/去伪造时间戳）

- [x] 4.1 Projects API 引入 `ProjectPublic` DTO（ts-rs 导出），从 `Deployment.public_config()` 映射生成；移除 `created_at/updated_at` 伪造语义
- [x] 4.2 前端适配 Projects DTO（hooks、页面、settings），并确保不重复建立 projects WS（复用全局 context，如果已存在）
- [x] 4.3 移除或降级 `sync_config_projects_to_db()`：从 `/api/config/reload` 移除同步调用；在 task create / attempt start 路径按需 `find_or_create`（以 config 为准）写入最小 project 行；并明确并测试“按需最小写入”规则
- [x] 4.4 增加回归测试：Projects 列表来自 YAML（DB 不作为配置源）；创建 task/attempt 时 DB project 行按需存在；项目名/策略以 YAML 为准（避免 DB 漂移影响）

Verification:
- `cargo test -p server projects tasks`
- `pnpm -C frontend run check`
