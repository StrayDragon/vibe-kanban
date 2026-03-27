## Why

测试与开发脚本是长期维护与稳定性的关键路径，目前仍存在可精简点：
- 本地/CI 运行会在仓库目录内生成临时 SQLite 文件或固定 `.e2e/` 目录，进程中断时容易残留脏状态，造成后续用例/开发行为抖动。
- E2E runner 脚本存在重复实现（dev vs just-run 两套），维护成本高且易漂移。
- 部分脚本依赖 `bash + jq + perl` 等系统工具，跨平台与 CI 环境一致性较差。
- Rust workspace 内各 crate 自建 env lock / EnvGuard / temp root / test DB harness，重复且容易遗漏，增加 flaky 与 UB 风险。

## What Changes

- `scripts/prepare-db.js`：临时 SQLite 移到 OS temp + 随机文件名/目录，避免污染工作区。
- `scripts/qa.sh`：去掉重复的 migration check，减少 CI 时间与维护面。
- 合并两套 E2E runner：统一成一个脚本（支持 `--mode=dev|just-run`），并将 `.e2e/` 改为每次运行唯一目录，teardown 强清理。
- `scripts/check-i18n.sh` 重写为单个 Node/TS 脚本，减少系统依赖。
- 抽一个统一的 Rust 测试支持 crate（例如 `crates/test-support`）：提供 env lock、EnvVarGuard、TempRoot、TestDb 等，逐步替换各 crate 的重复实现。

Goals:
- 减少 flaky 与脏状态残留，提升 CI 稳定性与本地体验。
- 降低脚本/测试工具链的环境依赖，提升可移植性。
- 通过统一 test harness 降低维护成本与重复代码。

Non-goals:
- 不改变产品功能行为。
- 不在本变更里重做所有 E2E 用例（只重构 runner/harness 与基础设施）。

Risks:
- runner/harness 重构可能引入短期 CI 失败或路径差异。
  - Mitigation: 渐进迁移；保留旧脚本一段时间作为 fallback（可选），并在 CI 中并行验证一两个周期后再移除。

Verification:
- `cargo test --workspace`
- `pnpm -C frontend run check`
- `node scripts/run-e2e.js --mode=dev`（或等价命令）

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `test-stability`: 明确禁止 repo-local 临时产物污染与 sleep/环境依赖导致的 flaky；统一测试 harness 与 E2E runner 的确定性策略。

## Impact

- `scripts/*`（prepare-db、qa、e2e runner、check-i18n）
- `playwright.config.ts` / `e2e/*`（runner 生命周期与临时目录）
- `crates/*` tests（EnvGuard/锁/临时目录/DB harness 统一）

