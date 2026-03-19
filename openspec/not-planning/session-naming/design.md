## 背景

Session 数据存储在 `sessions` 表中，并通过 `/api/sessions` 对外提供。Session 会在多个后端流程中创建（initial agent run、setup flows、PR flows 等），但当前 schema 只包含 `executor` 字符串与时间戳。

前端目前没有独立的「session 列表」UI；主要展示 execution processes 与 normalized logs。Processes 对话框是用户在多次运行之间切换/导航的最直接入口，最适合作为 session name 的首个展示位置。

## 目标 / 非目标

**目标：**
- 增加可选字段 `Session.name`，并满足：
  - 创建时若未提供则可自动生成
  - 用户可重命名
- 提供稳定的 API 用于重命名 session。
- 在 Processes 对话框中展示 session name，并支持重命名。

**非目标：**
- 不围绕 session 重做 attempt/task 的整体 UI。
- 本次不实现 session name 的全文检索。

## 决策

### 决策：在 `sessions` 表中新增可空列 `name`

通过 SeaORM migration 在 `sessions` 表新增 `name TEXT NULL`。为保持向后兼容，`name` 保持可选。

服务端校验规则：
- trim 前后空白。
- 空字符串 SHALL 被视为 `NULL`。
- 限制最大长度（默认：120 字符），避免 UI 被破坏。

### 决策：通过 `PATCH /api/sessions/:session_id` 重命名

新增接口：
- `PATCH /api/sessions/:session_id`
- Body：`{ "name": string | null }`
- Response：更新后的 `Session`

该接口只修改 session name，不影响 executor 或其他状态。

### 决策：自动命名按“尽力而为 + 分流程”实现

自动命名仅在未提供显式 name 时生效。

初始命名规则（默认）：
- coding agent 的初始 session：`Run: <task.title>`（截断至 80）
- setup helper sessions（Codex/Cursor/GH CLI setup）：`<Tool> Setup`
- fallback：`Session <short-id>`（由 UUID prefix 派生）

我们会在创建 session 的关键调用点上实现命名（当上下文可得，例如 task title、flow type）。若某处无法提供上下文，则可不传 name，并依赖 UI fallback。

### 决策：首个 UI 落点为 Processes 对话框

扩展 Processes 对话框以：
- 拉取当前 workspace/attempt 的 sessions
- 展示一个 session selector，优先显示 `name`（否则显示 fallback label）
- 为当前选中 session 提供重命名动作
- （可选）按选中的 session id 过滤 execution process 列表

这样无需更大规模的导航重构即可让功能落地可用。

## 风险 / 取舍

- **[创建 session 的调用点较多]** → 先在关键调用点做 best-effort 自动命名，不以覆盖率 100% 为阻塞条件。
- **[UI 复杂度]** → UI 保持轻量（selector + rename），避免重构。

## 迁移计划

1. 增加 DB migration + SeaORM entity/model 更新。
2. 更新 `/api/sessions` 返回包含 `name`。
3. 新增 rename endpoint。
4. 重新生成 TS types。
5. 更新 Processes 对话框展示/重命名 session name。

## 开放问题

- name 是否需要本地化？（默认：**否**，持久化内容保持英文。）
- 清空 name 时是否重新自动命名，还是仅展示 fallback？（默认：仅展示 fallback，不重写历史 name。）
