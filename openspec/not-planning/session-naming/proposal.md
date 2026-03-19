## 为什么

随着用户运行越来越多的 attempt 与 follow-up，一个 workspace 内可能包含多个
session。当前 session 只能通过不透明的 ID（以及最多一个 executor 字符串）来识别，
这会导致：

- 很难快速定位“发生了某件事的那一次 session”
- 难以区分 setup/工具类 session 与真正的 coding session
- UI 和工具里很难复用/引用某个具体的 session

上游 Vibe Kanban 通过「session 命名（可重命名 + 自动命名）」提升了导航与上下文可读性。

## 变更内容

- 为 `Session` 增加可选字段 `name`（DB + API + 生成的 TS types）。
- 增加 session 重命名接口，供 UI 修改 session 名称。
- 为常见创建流程实现“尽力而为”的自动命名（优先覆盖创建 attempt 时的 coding session；
  其他流程按可获得的上下文补齐），且**不得覆盖**用户显式提供的名称。
- 在 UI 中展示 session 名称（首个落点为 Processes 对话框），并支持用户重命名。

## 能力

### 新增能力

- `session-naming`：session 拥有人类可读名称，可自动设置且允许用户重命名。

### 变更的能力

<!-- 无 -->

## 影响范围

- `sessions` 表：新增字段与迁移（SeaORM migration + entity/model 更新）。
- 后端路由：`/api/sessions/*` 相关接口。
- 前端：Processes 对话框（`frontend/src/components/tasks/TaskDetails/*`）展示与重命名。
- Rust 类型变更后需运行 `pnpm run generate-types` 以更新 `shared/types.ts`。

## 目标 / 非目标

**目标：**
- 让用户在 UI 中更容易区分和引用不同 session。
- 保持向后兼容：`name` 为可选字段，不破坏现有消费者。

**非目标：**
- 本次不做 session 名称的全局搜索/索引（可后续补充）。
- 不改变 execution-process 语义或日志格式。

## 风险

- DB 迁移引入新列 → 保持 `NULL` 可用并向后兼容。
- UI 落点选择不明确 → 先落到 Processes 对话框，其他位置暂不扩散。

## 验证方式

- 创建新 attempt，确认最新 session 会在可获得上下文时自动生成名称。
- 在 UI 中重命名 session，刷新后名称仍然持久化。
- 运行 `pnpm run generate-types` + `pnpm run check` + `cargo test --workspace`。
