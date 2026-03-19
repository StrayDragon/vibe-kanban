## 1. Executor Profile + Config

- [ ] 1.1 在 Codex executor config 中新增 `plan: bool`
- [ ] 1.2 在 `crates/executors/default_profiles.json` 中新增 CODEX `PLAN` variant，并提供安全默认值（read-only sandbox + strict tool allowlist）

## 2. Plan-Only Enforcement

- [ ] 2.1 在 Codex app-server client 中实现 plan mode enforcement：拒绝 mutation tool calls 与命令执行
- [ ] 2.2 增加回归测试，验证 plan mode 下 mutation tools 会被拒绝

## 3. UI（Minimal）

- [ ] 3.1 确认 plan updates 会出现在现有 Todo panel（已归一化），并在合适位置增加小的 “Plan-only” label

## 4. Verification

- [ ] 4.1 运行 `pnpm run backend:check` 与 `pnpm run check`
- [ ] 4.2 运行 `cargo test --workspace`
