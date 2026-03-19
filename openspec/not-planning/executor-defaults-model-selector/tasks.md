## 1. 规范化 Profile ID 表示

- [ ] 1.1 在 `crates/config/src/schema.rs` 中归一化 `executor_profile.variant`：将 `"DEFAULT"`（任意大小写）转换为 `null`
- [ ] 1.2 更新 onboarding profile picker：DEFAULT 使用 `variant = null` 存储（避免写入 `"DEFAULT"`），文件：`frontend/src/components/dialogs/global/OnboardingDialog.tsx`
- [ ] 1.3 添加 config unit test，验证 `"DEFAULT"` 会被归一化为 `null`

## 2. 为 Attempts 暴露 Last Used Profile

- [ ] 2.1 扩展 `get_task_attempts_with_latest_session` 使用的 attempt/session summary DTO，增加可选字段 `executor_profile_id`（coding-agent，包含 variant）
- [ ] 2.2 从该 attempt/session 最新的 coding-agent execution process 填充该字段（不再从 `session.executor` 猜 variant）
- [ ] 2.3 重新生成 TS types（`pnpm run generate-types`），并修复前端编译错误（如有）

## 3. 统一 UI 默认选择逻辑

- [ ] 3.1 更新 `CreateAttemptDialog` 的默认 profile 解析逻辑：当 `executor_profile_id` 存在时优先使用
- [ ] 3.2 （可选）抽取一个小的共享 helper，用于默认 profile 解析，保证 TaskForm/CreateAttempt/Milestone workflows 的行为一致

## 4. Verification

- [ ] 4.1 运行 `pnpm run check` 与 `pnpm run lint`
- [ ] 4.2 运行 `pnpm run backend:check`
- [ ] 4.3 运行 `cargo test --workspace`
