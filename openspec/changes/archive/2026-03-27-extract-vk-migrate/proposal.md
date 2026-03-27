## Why

当前 `crates/server/src/legacy_migrations.rs` 体积巨大且职责混杂，并且通过 `server legacy ...` 子命令直接编译进主 `server` 二进制。这带来几个问题：
- 主服务的编译面与维护面被“deprecated 迁移工具”拖累（更慢、更复杂、更易出安全/bug）。
- 迁移工具包含大量文件 IO、YAML merge、备份、权限处理等边缘逻辑，长期维护成本高。
- deprecated 能力应当易于拆除，但当前拆除成本偏高（强耦合在 server 中）。

我们希望把迁移工具从主服务剥离成独立的 **operator CLI**：`vk`（后续也可扩展为统一入口），并强简化写入逻辑（默认只生成输出文件，用户手工合并），同时提供一段可复制的 AI prompt（Claude Code/Codex）帮助用户低摩擦完成迁移。

## What Changes

- **BREAKING**: 将 `server legacy ...` 迁移命令从主 `server` 二进制剥离，改为 `vk migrate ...` 子命令提供。
- **BREAKING**: 强简化 `--install`：默认仅输出迁移结果文件（例如 `config.migrated.<ts>.yaml` / `projects.migrated.<ts>.yaml` / `secret.env.migrated.<ts>`），不再做复杂的 YAML merge 写入；如需合并由用户手工或借助 AI 完成。
- 迁移逻辑模块化拆分（db projects / asset config / secrets / io），并尽可能复用 `crates/config` 的校验作为“唯一真相”。
- 为 `vk migrate prompt` 增加一键复制提示词：打印一段针对 Claude Code/Codex 的迁移提示词（指导用户如何把 legacy 文件合并到新 YAML）。

Goals:
- 让主 `server` 更小、更纯：不编译/不暴露 deprecated 迁移能力。
- 迁移工具易维护、易移除：模块化 + 最小写入行为。
- 用户迁移路径更低摩擦：输出文件 + AI prompt 指引。

Non-goals:
- 不在本变更中移除迁移功能本身（仍保留导出能力，但从 server 中剥离）。
- 不在运行时写用户配置（迁移工具是一次性 CLI）。

Risks:
- CLI 破坏性：用户脚本可能依赖 `server legacy ...`。
  - Mitigation: 在 release notes 说明替代命令；可短窗口保留旧命令但输出 deprecation + 退出码（可选）。
- 移除 merge 安装能力会改变体验。
  - Mitigation: 输出文件命名明确、提供 AI prompt 与手工合并指引。

Verification:
- `cargo test --workspace`
- 手动：`vk --help`、核心导出命令可运行、输出文件权限正确（secret 输出 0600）

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `crate-boundaries`: 主服务二进制不再包含 deprecated 迁移工具；迁移工具归属到独立边界（可选构建/可选分发）。

## Impact

- Rust workspace:
  - 新增 `vk` crate/bin（operator CLI）
  - `crates/server/src/main.rs` 移除 legacy 子命令 wiring
  - `crates/server/src/legacy_migrations.rs` 拆分/迁移
- 文档与脚本：安装/迁移说明需要更新
