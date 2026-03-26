# 配置迁移：从旧 DB 导出 projects → 写入 `config.yaml`

本项目已将 **projects / repos 设置**从 DB-backed 改为 **YAML file-first**。服务端不会写入 `config.yaml`；如需从旧 DB 迁移数据，请使用一次性脚本完成导出/合并。

> 说明：你如果在 `~/.config/vk/config.yaml` 里只看到了 `projects:`，这是正常的——该导出命令只处理 DB 里的 projects/repos。
> 旧版的“系统设置/默认 executor/profile/agent env”等并不在 DB 表里，而是位于 `asset_dir()/config.json` 与 `asset_dir()/profiles.json`，需要使用另一个迁移命令（见下文）。

## 一键迁移（推荐）

将 DB 中的 projects/repos 导出并合并到用户配置目录的 `config.yaml`（仅更新 `projects` 字段；**不导出 secrets**）：

```bash
server legacy export-db-projects-yaml --install
```

安全性说明：

- 若 `config.yaml` 已存在：脚本会先写入备份 `config.yaml.bak.<timestamp>`，再将 `projects` 合并写回。
- 若现有 `config.yaml` 不是合法 YAML（或顶层不是 mapping）：脚本不会覆盖它，而是写出 `projects.db-export.<timestamp>.yaml`，并提示你手动合并后再 reload。

预演（不落盘）：

```bash
server legacy export-db-projects-yaml --install --dry-run
```

## 只导出为文件/标准输出

导出 YAML 片段（仅 `projects:`）到文件：

```bash
server legacy export-db-projects-yaml --out /tmp/vk-projects.yaml
```

导出到标准输出：

```bash
server legacy export-db-projects-yaml --out -
```

## 路径解析（参考/排查）

脚本提供“打印解析后的路径”能力：

```bash
server legacy export-db-projects-yaml --print-paths
```

输出包含：

- `database_url` / `database_file`：DB 来源（优先 `DATABASE_URL`；否则使用 `${VIBE_ASSET_DIR:-default_asset_dir}/db.sqlite`）
- `config_dir` / `config_yaml` / `secret_env`：用户配置目录与目标文件（优先 `VK_CONFIG_DIR`；否则 OS 默认目录）

## secrets 迁移建议（重要）

- 脚本 **不会**导出任何 token/pat/key。
- 建议将 secrets 放入与 `config.yaml` 同级的 `secret.env`，再在 `config.yaml` 中通过 `{{secret.NAME}}` 引用。

示例：

`secret.env`
```env
GITHUB_PAT=...
OPENAI_API_KEY=...
```

`config.yaml`
```yaml
github:
  pat: "{{secret.GITHUB_PAT}}"
```

## 迁移后如何生效

编辑完成后执行：

```bash
curl -s -X POST http://localhost:<BACKEND_PORT>/api/config/reload
```

或等待（可选）文件监听自动 reload（如果启用）。

## 给其他人/其他 LLM 的 Prompt 模板

将下面这段 prompt 复制给协作者，并补充你机器上执行 `--print-paths` 的输出与导出/报错信息：

```text
你是 vibe-kanban(VK) 的迁移助手。我的目标是把旧版本保存在本机 sqlite DB 里的 projects/repos 配置迁移到用户配置目录的 config.yaml（YAML file-first），并把 secrets 放到同级的 secret.env，通过 {{secret.X}} 引用。

已知约束：
- 服务端/前端不允许写入 config.yaml；只能用一次性脚本/CLI 迁移。
- 迁移只处理 projects/repos；不导出 secrets。
- config dir 由 VK_CONFIG_DIR 覆盖，否则使用 OS 默认用户配置目录。
- DB 由 DATABASE_URL 覆盖，否则使用 VIBE_ASSET_DIR/default_asset_dir 下的 db.sqlite。

请基于以下信息给出可执行步骤（含回滚/备份）：
1) `server legacy export-db-projects-yaml --print-paths` 的输出：<粘贴>
2) 我希望使用的迁移方式：`--install`（合并写入 config.yaml）/ `--out`（导出片段手动合并）
3) 如果命令失败/导出为空/路径不对，请给出排查点与替代方案。
```

## 源码构建（开发者）

如果你是从源码运行（没有全局 `server` 可执行文件），用：

```bash
cargo run --bin server -- legacy export-db-projects-yaml --install
```

注意：旧的 `cargo run --bin export_db_projects_yaml -- ...` 入口属于过渡期遗留，将在未来版本移除。

---

# 迁移 legacy `config.json` / `profiles.json` → `config.yaml` + `secret.env`

如果你从旧版本升级，并且旧版曾在 `asset_dir()` 下写入：

- `config.json`（系统设置，如 language/theme/notifications/default executor profile 等）
- `profiles.json`（executor profiles overrides，常包含 agent 的 `env`，其中可能含 token）

可以使用一次性迁移工具将它们导出/合并到用户配置目录：

```bash
server legacy export-asset-config-yaml --install
```

安全性说明（重要）：

- 迁移会将 **疑似敏感** 的 env 值写入与 `config.yaml` 同级的 `secret.env`，并在 YAML 中替换为 `{{secret.NAME}}`。
- 迁移只保留当前构建支持的 executors（默认仅 `CLAUDE_CODE` + `CODEX`）；其它（例如 `FAKE_AGENT`）会被丢弃并打印 warning。
- 若 `config.yaml` / `secret.env` 已存在：会先写入备份 `*.bak.<timestamp>`，再合并写回。

预演（不落盘）：

```bash
server legacy export-asset-config-yaml --install --dry-run
```

路径解析（参考/排查）：

```bash
server legacy export-asset-config-yaml --print-paths
```
