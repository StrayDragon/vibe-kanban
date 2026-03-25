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

- `${NAME}`
- `${NAME:-default}`

当 `${NAME}` 缺失且未提供默认值时，配置校验失败；reload 会保留 last-known-good 配置并记录 last error。

## YAML LSP（校验 / Hover / 补全）

在 `config.yaml` 顶部添加：

```yaml
# yaml-language-server: $schema=./config.schema.json
```

启动 VK 后会在同一目录生成 `config.schema.json`，编辑器即可使用该 schema 提供校验与补全。

## Reload / Status

- 状态：`GET /api/config/status`
- 触发 reload：`POST /api/config/reload`

