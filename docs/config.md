# VK 配置（YAML + LSP）

VK 采用 **file-first** 的配置模型：以 OS 用户配置目录下的 `config.yaml` 作为唯一事实来源，不再通过数据库或 Settings API 写入配置。

## 配置目录

- 默认（Linux/macOS）：`~/.config/vk/`
- Windows：`%APPDATA%\\vk\\`
- 覆盖：设置环境变量 `VK_CONFIG_DIR=/path/to/dir`

## 文件约定

- `config.yaml`：主配置文件（YAML）
- `secret.env`：可选，dotenv 形式的 secrets overlay
- `config.schema.json`：运行时自动生成的 JSON Schema（用于 YAML LSP）

## secrets 与模板注入

VK 会自动加载 `secret.env`，并按以下优先级解析 `config.yaml` 中字符串值的模板：

1. `secret.env`
2. 进程 / 系统环境变量

支持的模板：

- `{{env.NAME}}`
- `{{env.NAME:-default}}`
- `{{secret.NAME}}`

说明：

- `{{env.NAME}}` 会按 `secret.env` → 系统 env 的顺序解析（即 `secret.env` 覆盖系统变量）。
- `{{secret.NAME}}` 仅从 `secret.env` 解析。
- 当缺失且未提供默认值时，配置校验失败；reload 会保留 last-known-good 配置并记录 last error。

## YAML LSP（校验 / Hover / 补全）

在 `config.yaml` 顶部添加：

```yaml
# yaml-language-server: $schema=./config.schema.json
```

启动 VK 后会在同一目录生成 `config.schema.json`，编辑器即可使用该 schema 提供校验与补全。

## Reload / Status

- 状态：`GET /api/config/status`
- 触发 reload：`POST /api/config/reload`

## 从旧 DB 导出 projects（可选）

如果你从旧版本升级，且本地 DB 仍保存了 project/repo 的设置，可以使用一次性导出工具生成可被 loader 读取的 YAML。

推荐直接将 DB 中的 `projects` 合并写入用户配置目录的 `config.yaml`：

```bash
server legacy export-db-projects-yaml --install
```

（预演不落盘：`--install --dry-run`；打印解析后的路径：`--print-paths`。）

```bash
server legacy export-db-projects-yaml --out /tmp/vk-projects.yaml
```

该导出：

- 只生成 `projects:` 片段（不包含 secrets）
- 会对明显无效的脚本/路径做清理并打印 warning（保证输出可被 loader 读取）

将导出的 `projects:` 合并到 `~/.config/vk/config.yaml`（或 `VK_CONFIG_DIR`），然后执行 `POST /api/config/reload` 生效。

更多迁移细节（含 prompt 模板）：见 `docs/config-migration.md`。

## 从 legacy `config.json` / `profiles.json` 迁移（可选）

如果你升级自旧版本，并且旧版曾在 `asset_dir()` 下保存系统设置与 profiles overrides，可以运行：

```bash
server legacy export-asset-config-yaml --install
```

该工具会把 secrets 写入同级 `secret.env`，并在 YAML 中替换为 `{{secret.NAME}}`（不会把 token 明文写进 YAML）。
