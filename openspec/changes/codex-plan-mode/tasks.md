## 1. Executor Profile + Config

- [ ] 1.1 Add `plan: bool` to the Codex executor config
- [ ] 1.2 Add CODEX `PLAN` variant to `crates/executors/default_profiles.json`
      with safe defaults (read-only sandbox + strict tool allowlist)

## 2. Plan-Only Enforcement

- [ ] 2.1 Implement plan-mode enforcement in the Codex app-server client:
      reject mutation tool calls and command execution
- [ ] 2.2 Add regression tests that verify mutation tools are denied in plan
      mode

## 3. UI (Minimal)

- [ ] 3.1 Ensure plan updates appear in the existing Todo panel (already
      normalized) and add a small "Plan-only" label where appropriate

## 4. Verification

- [ ] 4.1 Run `pnpm run backend:check` and `pnpm run check`
- [ ] 4.2 Run `cargo test --workspace`

