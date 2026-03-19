## 为什么

我们有多个入口会选择“编码代理 + 配置（用户感知为 model）”：Onboarding、Settings → Agents、
任务/attempt 创建对话框、以及 milestone 工作流的默认值。当前这些流程的默认选择逻辑并不完全统一，
而且数据表示也不一致（例如有的路径会写入 `variant: "DEFAULT"`，有的路径使用 `null`）。

这会造成：

- UX 困惑（“为什么它选了这个模型/配置？”）
- 默认值逻辑难以推理与维护
- UI 高亮/持久化可能异常（同一语义被两种表示方式打散）

## 变更内容

- 定义并强制统一 `ExecutorProfileId.variant` 的规范表示（canonical representation）：
  - `null`（缺失）表示 DEFAULT
  - `"DEFAULT"`（忽略大小写、允许前后空白）在边界处统一归一化为 `null`
- 统一新 attempt 的默认 executor profile 解析顺序：
  - milestone 节点锁定配置 > 用户在对话框里选择 > 该 attempt/任务最近一次 coding-agent 使用的 profile（含 variant） > 用户系统默认值
- 在 attempt 列表/摘要中暴露“最近一次 coding-agent 使用的 executor profile”（executor + variant），让 UI 默认选择无需猜测。
- Onboarding 与各类对话框复用一致的 selector 行为，避免各处各写一套 “variant 下拉” 逻辑。

## 能力

### 新增能力

- `executor-profile-defaulting`：UI + API 各入口对 executor profile 的默认选择一致，
  且 profile ID 表示统一规范化。

### 变更的能力

<!-- 无 -->

## 影响范围

- 后端：attempt/session summary DTO（用于 attempt 创建 UI）。
- 配置：`crates/config/src/schema.rs` 对 executor profile variant 的归一化。
- 前端：Onboarding profile picker 与 Create Attempt 默认选择逻辑。
- 类型：如 Rust DTO 有变更，需要 `pnpm run generate-types` 更新 `shared/types.ts`。

## 目标 / 非目标

**目标：**
- 让 DEFAULT 在所有地方表现一致（持久化存储为 `variant = null`）。
- 让默认选择可预测、跨入口一致。

**非目标：**
- 本次不做新的 executor/model discovery UX。
- 不改变 follow-up 的 executor 切换语义（仍然只允许变更 variant）。

## 风险

- 为派生“last used profile”可能需要额外 DB/API 查询 → 初期将范围限定为“最近一次 coding-agent 过程”，
  必要时再做缓存/限制或批量化优化。

## 验证方式

- 旧 config 中 `variant: "DEFAULT"` 能被加载并在保存后写回为 `variant: null`。
- Create Attempt 对话框在存在历史 coding-agent 运行时，会优先预选中同一个 profile（含 variant）。
- 运行 `pnpm run check`、`pnpm run lint`、`pnpm run backend:check`、`cargo test --workspace`。
