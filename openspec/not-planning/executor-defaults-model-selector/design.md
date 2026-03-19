## 背景

我们使用 `ExecutorProfileId`（`{ executor, variant }`）表示用户选择的 agent + configuration，其中 `variant` 为可选字段，缺失时表示 DEFAULT configuration。

当前存在两类问题：

1) **表示漂移（representation drift）**

- `frontend/src/components/dialogs/global/OnboardingDialog.tsx` 直接基于 profile keys 构建 variant 下拉，可能写入 `variant = "DEFAULT"`。
- `frontend/src/components/tasks/ConfigSelector.tsx` 将 DEFAULT 视为 `variant = null`。
- `crates/config/src/schema.rs` 会把空 variant 归一化为 `null`，但不会把 `"DEFAULT"` → `null`。

这会导致 UI 高亮不一致，并让“存储值到底代表什么”更难推理。

2) **默认选择漂移（defaulting drift）**

- `frontend/src/components/dialogs/tasks/CreateAttemptDialog.tsx` 会根据 latest attempt 选择默认值，但它只有 `attempt.session.executor`，无法可靠还原上次使用的 *variant*（因为 “TaskAttempt doesn't store it”）。
- 后端在 execution process actions（`CodingAgentInitialRequest` / `CodingAgentFollowUpRequest`）里已经记录了完整的 `ExecutorProfileId`，并提供了诸如 `ExecutionProcess::latest_executor_profile_for_session(...)` 的 helper。

最终结果是：用户以为自己选的是 “Model Selector”，而系统实际默认选择的 profile 可能在不同入口出现偏差。

## 目标 / 非目标

**目标：**
- 规范化 DEFAULT 的表示：DEFAULT 统一存储为 `variant = null`。
- 为新 attempt 提供单一、文档化的默认 executor profile 解析顺序。
- 通过在 Create Attempt 对话框可用的数据中暴露 last used coding-agent profile，消除对 variant 的“猜测”。

**非目标：**
- 不做新的 model discovery UX（例如拉取远端 model 列表等）。
- 不改变 follow-up 中“中途切换 executor”的行为（variant-only remains）。
- 不做 profiles 格式的大重构（保持 `crates/executors/default_profiles.json` 语义不变）。

## 决策

### 决策：`variant = null` 是 DEFAULT 的 canonical 表示

我们在 config、API、UI 全链路将 DEFAULT configuration 统一表示为 `variant = null`。

边界归一化规则：
- empty / whitespace variant → `null`（已存在）
- `"DEFAULT"`（case-insensitive, trimmed）→ `null`（新增）

**备选方案：**保留 `"DEFAULT"` 作为合法存储值，并更新所有 UI 路径。否决原因：当前 `null` 已是更常见的表示，且对 serde 更友好（`skip_serializing_if`）。

### 决策：Create Attempt 的默认选择采用显式优先级顺序

Create Attempt UI 使用以下 precedence order：

1. milestone node override（locked）
2. 用户在对话框里的选择
3. last used coding-agent `executor_profile_id`（包含 variant）
4. 用户系统默认 `config.executor_profile`

**备选方案：**总是使用 `config.executor_profile`。否决原因：会丢失很多用户期待的 “repeat the last run” 工作流。

### 决策：通过 API 暴露 last used coding-agent profile（派生，不持久化）

我们会在 Create Attempt 对话框使用的 attempt/session summary payload 上新增一个可选字段，用于携带该 attempt 的 last used coding-agent `ExecutorProfileId`。

初期实现：从该 session/workspace 最新的 coding-agent execution process 派生。

**备选方案（DB 变更）：**在 session 创建时把 `executor_profile_id`（或 variant 列）持久化到 `sessions` 表。该方案需要 DB migration 和回填语义，先暂缓；若派生查询成本过高再考虑。

## 风险 / 取舍

- **[额外 DB 开销]** 为每个 attempt 派生 last-used profile 可能产生 N+1 查询。缓解：范围限定为“最新的 coding-agent process”，必要时只为 latest attempt 计算。
- **[遗留 config 值]** 既有 config 可能存了 `"DEFAULT"`。缓解：在 `Config::normalized()` 中归一化，并补充 unit test。

## 迁移计划

1. Config：将 `executor_profile.variant` 的 `"DEFAULT"` → `null`。
2. Frontend：更新 onboarding selector，DEFAULT 写入 `null`。
3. Backend：扩展 attempt/session summary DTO，加入派生的 `executor_profile_id`（来自 execution processes）。
4. Frontend：更新 Create Attempt 默认逻辑使用新字段（不再猜 variant）。
5. 如 Rust DTO 发生变更，重新生成 TypeScript types，并运行 checks/tests。

## 开放问题

- 当 last used profile 与 system default 不同，默认选择应更偏向哪一个？（当前默认：prefer last used）
- 将来是否需要在 `sessions` 上持久化 `executor_profile_id` 以实现 O(1) 读取？（默认：**否**，仅在 profiling 证明必要时再做。）
