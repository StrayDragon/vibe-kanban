## 1. prepare-db 临时 SQLite 去工作区污染

- [x] 1.1 修改 `scripts/prepare-db.js`：临时 sqlite 放 OS temp + 随机路径（或唯一目录），避免在 repo 内创建固定文件
- [x] 1.2 增加清理逻辑（成功/失败/中断后尽量清理临时文件）

Verification:
- `pnpm run prepare-db`

## 2. QA 脚本去重

- [x] 2.1 移除 `scripts/qa.sh` 中重复的 migration check（合并 `prepare-db:check` 与 `remote:prepare-db:check` 逻辑）
- [x] 2.2 确保 CI 仍覆盖关键检查（types、lint、tests）

Verification:
- `pnpm run qa`（或 CI 对应命令）

## 3. E2E 生命周期交给 Playwright（webServer + Setup/Teardown）

- [x] 3.1 配置 `playwright.config.ts` 的 `webServer`：根据 `VK_E2E_MODE=dev|just-run` 选择启动命令，并设置 `url/timeout` 等待就绪
- [x] 3.2 新增 `e2e/global-setup.ts` / `e2e/global-teardown.ts`：创建每次唯一运行目录（OS temp 或 `.e2e/<run-id>`），生成 config/projects/测试 repo，并在 teardown 强清理
- [x] 3.3 删除 `scripts/run-e2e.js` 与 `scripts/run-e2e-just-run.js`，并更新 `package.json` scripts：用环境变量选择模式后直接运行 `playwright test`

Verification:
- `pnpm run e2e:test`
- `VK_E2E_MODE=just-run pnpm run e2e:test`

## 4. check-i18n 改为 Node/TS 单脚本

- [x] 4.1 将 `scripts/check-i18n.sh` 重写为单个 Node/TS 脚本，移除 jq/perl/diff 依赖
- [x] 4.2 更新 `package.json` scripts 与 CI 调用点

Verification:
- `pnpm run check-i18n`（按仓库实际脚本名）

## 5. Rust tests 统一 harness（crates/test-support）

- [x] 5.1 新增 `crates/test-support`：提供 env lock、EnvVarGuard（RAII）、TempRoot、TestDb 等通用能力
- [x] 5.2 迁移现有重复实现：优先 `crates/server`/`crates/config`/`crates/app-runtime`（减少 flaky 与重复）
- [x] 5.3 修复遗留 guard：让需要 env lock 的 guard 自己拿锁（避免调用方遗漏）

Verification:
- `cargo test --workspace`
