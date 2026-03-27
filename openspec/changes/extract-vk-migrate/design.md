## Context

迁移工具本质上是一次性 CLI，不应成为 core server 的长期负担。当前 legacy_migrations 代码在 server crate 内：
- 与 server 编译/依赖强绑定
- 包含大量边缘 IO/merge 行为
- 很难在未来“干净地删除 deprecated 命令”

## Goals / Non-Goals

**Goals:**
- 引入独立二进制 `vk-migrate` 承载迁移相关命令。
- 主 `server` 不再编译/暴露 `legacy` 子命令（更小、更纯）。
- `--install` 行为强简化：默认仅输出文件，不做 merge 写入。
- 提供 `vk-migrate prompt` 输出 AI 迁移提示词，降低用户迁移成本。
- 迁移逻辑尽可能复用 `crates/config` 校验（唯一真相），减少规则分叉。

**Non-Goals:**
- 不改变 YAML 配置体系（仍是 config.yaml + projects.yaml/projects.d + secret.env）。
- 不在运行时提供任何写配置 API。

## Decisions

1. **迁移工具剥离为独立 crate/bin**
   - 选择：新增 `crates/vk-migrate`（或 `crates/migrate`）并提供 `vk-migrate` 二进制。
   - 选择：主 `server` 移除 legacy 子命令 wiring，避免编译/依赖拖累。

2. **`--install` 改为输出文件（output-only）**
   - 选择：迁移命令默认只输出文件到 config dir（或指定 out path），不再做 YAML merge。\n     输出文件命名带时间戳，避免覆盖：`config.migrated.<slug>.yaml` 等。
   - 原因：merge 行为复杂且易出错；output-only 更可预测，也符合“只读 core”方向。

3. **提供 AI prompt 帮助用户合并**
   - 选择：`vk-migrate prompt` 生成一段 prompt，包含：\n     - 新旧文件路径\n     - 合并目标（projects.yaml、config.yaml）\n     - 注意事项（不要提交 secrets、检查 schema、reload 验证）\n     - 推荐命令（Claude Code/Codex）\n   - 原因：用最小产品方式覆盖“用户不想手工合并”的真实需求。

4. **模块化拆分 + 复用 config 校验**
   - 选择：将 legacy_migrations 拆为 `db_projects` / `asset_config` / `secrets` / `io` 等模块。\n     生成 YAML 后统一调用 `config::try_load_config_from_file`（或新 pair loader）验证可加载。
   - 原因：减少重复校验与 drift；未来删命令可删模块。

## Risks / Trade-offs

- [CLI breaking] 旧命令路径改变。
  - 缓解：清晰 deprecation 信息；文档更新；可选短窗口保留旧命令但仅打印提示并退出。
- [用户体验] 不再自动 merge 写入。
  - 缓解：输出文件 + AI prompt + 文档；并确保输出结构尽量贴近最终可用格式。

## Migration Plan

1. 新增 `vk-migrate` crate/bin，先把现有命令原样搬过去（保持行为）。
2. server 移除 legacy 子命令 wiring，确保 core server 仍可编译运行。
3. 强简化 `--install`：改为 output-only，增加时间戳命名与权限控制。
4. 增加 `prompt` 子命令与文档。
5. 收敛测试：迁移工具测试隔离/feature-gated，只保留关键回归。

## Open Questions

- `vk-migrate` 是否也承载 schema upsert（与 config/schema 相关 CLI）？可作为后续合并点。

