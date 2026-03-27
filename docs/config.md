# VK 配置（YAML + LSP）

VK 采用 **file-first** 的配置模型：以 OS 用户配置目录下的 YAML 文件作为唯一事实来源，不再通过数据库或 Settings API 写入配置。

## 配置目录

- 默认（Linux/macOS）：`~/.config/vk/`
- Windows：`%APPDATA%\\vk\\`
- 覆盖：设置环境变量 `VK_CONFIG_DIR=/path/to/dir`

## 文件约定

- `config.yaml`：主配置文件（用户/执行器等相对稳定配置）
- `projects.yaml`：项目/仓库配置（推荐）
- `projects.d/*.yaml|yml`：可选，把项目配置拆分成多个文件（会按路径排序后合并加载）
- `secret.env`：可选，dotenv 形式的 secrets overlay
- `config.schema.json`：JSON Schema（用于 `config.yaml` 的 YAML LSP）
- `projects.schema.json`：JSON Schema（用于 `projects.yaml` / `projects.d/*` 的 YAML LSP）

## secrets 与模板注入

VK 会自动加载 `secret.env`，并按以下优先级解析 **白名单字段** 中的模板：

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
- 模板仅允许出现在明确白名单字段中（例如 token/script/executor profile env 等）。如果在非白名单字段中写入 `{{...}}`，配置校验会 fail-closed 并给出字段路径与迁移提示。
- 具体哪些字段支持模板：以 `config.schema.json` / `projects.schema.json` 的字段描述为准。

## YAML LSP（校验 / Hover / 补全）

在 `config.yaml` 顶部添加：

```yaml
# yaml-language-server: $schema=./config.schema.json
```

在 `projects.yaml`（或 `projects.d/*.yaml|yml`）顶部添加：

```yaml
# yaml-language-server: $schema=./projects.schema.json
```

运行以下命令在同一目录生成/更新 `config.schema.json` 与 `projects.schema.json`，编辑器即可使用这些 schema 提供校验与补全：

```bash
vk config schema upsert
```

## Reload / Status

- 状态：`GET /api/config/status`
- 触发 reload：`POST /api/config/reload`

## 从旧 DB 导出 projects（可选）

如果你从旧版本升级，且本地 DB 仍保存了 project/repo 的设置，可以使用一次性导出工具生成可被 loader 读取的 YAML。

推荐先生成一个输出文件，再手工/AI 合并到 `projects.yaml`（或拆分到 `projects.d/*.yaml`）：

```bash
vk migrate export-db-projects-yaml --install
```

（预演不落盘：`--install --dry-run`；打印解析后的路径：`--print-paths`。）

```bash
vk migrate export-db-projects-yaml --out /tmp/vk-projects.yaml
```

该导出：

- 只生成 `projects:` 片段（不包含 secrets）
- 会对明显无效的脚本/路径做清理并打印 warning（保证输出可被 loader 读取）

该命令会在 config dir 写出 `projects.migrated.<timestamp>.yaml`（不会覆盖现有 `projects.yaml`）。将其内容合并到 `projects.yaml` / `projects.d/*.yaml` 后，执行 `POST /api/config/reload` 生效。

> 注意：该 DB 导出只处理 projects/repos。旧版的“系统设置/默认 executor/profile/agent env”等不在 DB 表里，而在 `asset_dir()/config.json` 与 `asset_dir()/profiles.json`，需要使用下文的 asset 迁移命令。

更多迁移细节（含 prompt 模板）：见 `docs/config-migration.md`。

## 从 legacy `config.json` / `profiles.json` 迁移（可选）

如果你升级自旧版本，并且旧版曾在 `asset_dir()` 下保存系统设置与 profiles overrides，可以运行：

```bash
vk migrate export-asset-config-yaml --install
```

该工具会把 secrets 写入 `secret.env.migrated.<timestamp>`，并在 YAML 中替换为 `{{secret.NAME}}`（不会把 token 明文写进 YAML）。

`--install` 会写出 `config.migrated.<timestamp>.yaml` 与 `secret.env.migrated.<timestamp>`（不会覆盖现有 `config.yaml` / `secret.env`）。将其内容合并到目标文件后再 reload。
