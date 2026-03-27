## 1. 新增 `vk-migrate` crate/bin（迁移工具独立化）

- [ ] 1.1 新增 Rust crate/bin：`vk-migrate`（独立于 `server`），提供基础 `--help` 与子命令骨架
- [ ] 1.2 将 `crates/server/src/legacy_migrations.rs` 迁移逻辑搬迁到 `vk-migrate`（可按 `db_projects` / `asset_config` / `secrets` / `io` 拆模块），确保不再需要通过 `server` crate 访问实现
- [ ] 1.3 清理 `server` crate 内 legacy 迁移相关 module/export（避免 server runtime 依赖迁移实现）

Verification:
- `cargo check -p vk-migrate`
- `cargo check -p server`

## 2. 移除 `server legacy ...` 命令入口（Breaking）

- [ ] 2.1 `crates/server/src/main.rs` 删除 legacy CLI help 文案与 legacy args 分支；`server --help` 不再出现 `legacy` 命令组
- [ ] 2.2 对历史用法给出可操作的报错指引：当用户传入 legacy 参数时，输出“请改用 vk-migrate …”并返回非 0（不做兼容执行）

Verification:
- `cargo run -p server -- --help`（不显示 legacy）
- `cargo run -p server -- legacy export-db-projects-yaml --help`（明确指引 vk-migrate，且非 0 退出）

## 3. 强简化 `--install` 为 output-only（不再 merge 写入）

- [ ] 3.1 `vk-migrate export-db-projects-yaml`：`--install` 改为仅输出新文件（例如 `projects.migrated.<timestamp>.yaml`），不再读取/merge/覆盖现有 `projects.yaml`
- [ ] 3.2 `vk-migrate export-asset-config-yaml`：`--install` 改为仅输出新文件（例如 `config.migrated.<timestamp>.yaml` + `secret.env.migrated.<timestamp>`），不再读取/merge/覆盖现有 `config.yaml`/`secret.env`
- [ ] 3.3 明确输出目录规则（优先 `VK_CONFIG_DIR`，否则 OS config dir），并保留 `--out <path>` / `--out -` 行为（stdout）与 `--print-paths`
- [ ] 3.4 更新 help 文案与示例，确保用户理解“生成文件 + 手工/AI 合并 + reload”的新流程

Verification:
- `cargo run -p vk-migrate -- --help`
- 手动：运行两条 export 命令的 `--install --dry-run`，确认不会尝试写入/备份现有配置文件

## 4. 增加 `vk-migrate prompt`（一键复制 AI 合并提示词）

- [ ] 4.1 新增 `vk-migrate prompt` 子命令：输出可直接复制给 Claude Code/Codex 的迁移合并提示词
- [ ] 4.2 prompt 内容至少包含：输入文件路径（legacy + migrated 输出）、合并目标文件（`config.yaml`/`projects.yaml`/`secret.env`）、安全注意事项（不要提交 secrets）、完成后验证步骤（schema 校验/`/api/config/reload`）

Verification:
- `cargo run -p vk-migrate -- prompt`（输出稳定、可复制）

## 5. 权限与临时文件安全（secret 输出 0600）

- [ ] 5.1 `vk-migrate` 对所有 secret 输出文件使用安全原子写入：临时文件与最终文件权限均为 `0600`（Unix）；避免生成权限宽松的备份/临时 secret 文件
- [ ] 5.2 增加回归测试：在 Unix 下写出 migrated secret 文件后检查 mode=0600（允许在非 Unix 下跳过）

Verification:
- `cargo test -p vk-migrate`

## 6. 测试搬迁与文档更新

- [ ] 6.1 将 legacy_migrations 相关测试从 `server` 迁移到 `vk-migrate`，并按 output-only 新语义更新/删除旧的 merge/backup 测试
- [ ] 6.2 更新文档/指引（README 或 docs）：说明 `server legacy ...` 已移除，迁移请使用 `vk-migrate ...`；并补充推荐工作流（生成 migrated 文件 -> 合并 -> reload）

Verification:
- `cargo test --workspace`

