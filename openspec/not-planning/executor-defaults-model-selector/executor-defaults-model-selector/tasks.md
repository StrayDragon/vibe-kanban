## 1. Canonical Profile ID Representation

- [ ] 1.1 Normalize `executor_profile.variant` so `"DEFAULT"` (any case) becomes
      `null` in `crates/config/src/schema.rs`
- [ ] 1.2 Update onboarding profile picker to store `variant = null` for DEFAULT
      (avoid writing `"DEFAULT"`) in
      `frontend/src/components/dialogs/global/OnboardingDialog.tsx`
- [ ] 1.3 Add a config unit test that verifies `"DEFAULT"` is normalized to
      `null`

## 2. Expose Last Used Profile for Attempts

- [ ] 2.1 Extend the attempt/session summary DTO used by
      `get_task_attempts_with_latest_session` to include an optional
      `executor_profile_id` (coding-agent, including variant)
- [ ] 2.2 Populate the field from the latest coding-agent execution process for
      that attempt/session (no variant guessing from `session.executor`)
- [ ] 2.3 Regenerate TS types (`pnpm run generate-types`) and update any
      frontend compile errors

## 3. Unify Defaulting in UI

- [ ] 3.1 Update `CreateAttemptDialog` default profile resolution to use the
      new `executor_profile_id` field when present
- [ ] 3.2 (Optional) Extract a small shared helper for default profile
      resolution so TaskForm/CreateAttempt/Milestone workflows stay consistent

## 4. Verification

- [ ] 4.1 Run `pnpm run check` and `pnpm run lint`
- [ ] 4.2 Run `pnpm run backend:check`
- [ ] 4.3 Run `cargo test --workspace`

