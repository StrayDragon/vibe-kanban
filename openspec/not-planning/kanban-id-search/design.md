## 背景

Project Kanban 页面（`frontend/src/pages/ProjectTasks.tsx`）目前只基于 title/description 在内存里做过滤。后端 task model 暴露了 UUID `id`，但没有把 DB 中稳定的数字主键暴露出来，也没有提供 short ID。

## 目标 / 非目标

**目标：**
- 为 task 提供稳定的 `number`（numeric identifier）与 `short_id`。
- 扩展看板搜索：除 title/description 外，还匹配 `number`、`short_id`（以及 UUID prefix）。
- 在 task cards 中展示 `#<number>`，便于引用。

**非目标：**
- 本次不改服务端 search endpoint；仍是客户端对 stream task list 的过滤。
- 本次不实现 per-project numbering scheme。

## 决策

### 决策：使用现有 DB primary key 作为 `Task.number`

`tasks` 表已有 integer primary key（`tasks.id`）。我们将它在 API responses 中暴露为 `Task.number`。该值稳定，且无需新增 schema。

### 决策：`Task.short_id` 定义为 UUID prefix

`short_id` 取 task UUID 的前 8 个字符，全部小写。这样 deterministic 且无需额外存储。

### 决策：搜索匹配规则

给定用户输入 query `q`（trim 后）：

1. 若 `q` 匹配 `^#?\\d+$`，视为 number 搜索，匹配 `task.number == parsed(q)`。
2. 否则，匹配以下任一：
   - `task.short_id` contains `q`（case-insensitive），或
   - `task.id`（UUID string）contains `q`（case-insensitive），或
   - title/description contains `q`（保持现有行为）

这样规则可预测，避免“数字部分匹配”带来的意外行为。

### 决策：UI 展示

在 kanban card 与 task detail header 以小 badge 展示 `#<number>`。默认不展示 `short_id`（仅用于搜索），避免 UI 噪音。

## 风险 / 取舍

- **[number 的语义]** 全局编号 vs per-project 编号 → 先使用全局 numeric ID；仅在确有需求时再考虑 per-project sequence。
- **[UX 杂讯]** → 默认只展示 `#<number>`。

## 迁移计划

1. Backend：在 task DTOs 中新增 `number` 与 `short_id`，并重新生成 TS types。
2. Frontend：更新 kanban 搜索匹配逻辑。
3. Frontend：在 task cards 中展示 `#<number>`。

## 开放问题

- 是否也支持 `T-123` 之类前缀？（默认：**否**，只支持 `#123` 与 `123`。）
- number 匹配是否支持 partial match？（默认：**否**，仅精确匹配。）
