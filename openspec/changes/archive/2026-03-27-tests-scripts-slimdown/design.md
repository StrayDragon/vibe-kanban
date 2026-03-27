## Context

测试基础设施的目标是“确定性 + 可移植 + 易维护”。当前仓库存在一些典型反模式：
- 固定路径临时目录/DB 文件（进程被 kill 后残留，污染下一次运行）
- 重复 runner 脚本（同逻辑多份 copy，长期漂移）
- 强依赖系统工具链（jq/perl/diff），跨平台成本高
- Rust tests 分散的 env lock/guard（容易遗漏，panic 时污染后续用例）

## Goals / Non-Goals

**Goals:**
- 所有临时产物默认写入 OS temp（或唯一目录），且 teardown 可强清理。
- 合并 E2E runner 与生命周期管理，减少自管进程/等待/信号处理。
- 脚本尽量使用 Node/TS 单栈，减少系统依赖。
- Rust tests 抽统一 harness，避免重复与遗漏。

**Non-Goals:**
- 不改变 E2E 断言逻辑与业务用例（只重构 runner/harness）。

## Decisions

1. **prepare-db 临时 SQLite 使用 OS temp + 随机路径**
   - 选择：使用 `mkdtemp` 风格目录或随机文件名，避免在 repo 内写入 `prepare_db.sqlite`。

2. **E2E 生命周期完全交给 Playwright（webServer + globalSetup/Teardown）**
   - 选择：使用 Playwright 的 `webServer` 启动被测服务（dev/just-run 由环境变量选择），并通过 `globalSetup/globalTeardown` 负责：\n     - 创建唯一运行目录（OS temp 或 `.e2e/<run-id>`）\n     - 生成配置/资产/测试 repo\n     - 注入 `VK_CONFIG_DIR`/`VIBE_ASSET_DIR`/端口等环境变量\n     - teardown 强清理，避免脏状态\n   - 原因：减少自写进程管理/等待/信号处理代码，降低 flaky 与长期漂移。

3. **check-i18n 改为 Node/TS**
   - 选择：将 jq/perl/diff 的逻辑内聚到 Node 脚本，保证跨平台一致性。

4. **Rust tests 抽统一 test-support crate**
   - 选择：新增 `crates/test-support` 提供：\n     - 全局 env lock\n     - EnvVarGuard（RAII）\n     - TempRoot（唯一临时目录 + Drop 清理）\n     - TestDb（sqlite url/文件、默认 env 注入、禁用后台任务等）\n     逐步替换各 crate 的重复实现。

## Risks / Trade-offs

- [迁移成本] 各 crate tests 迁移需要逐步替换。
  - 缓解：先引入 test-support，再按 flaky/重复最多的 crate 优先迁移。
- [E2E runner 行为差异] 端口分配/等待策略改变可能触发短期不稳定。
  - 缓解：保留现有 waitForHttpOk 逻辑并加更清晰超时诊断；必要时并行跑旧 runner 一段窗口。

## Migration Plan

1. prepare-db 与 E2E 目录唯一化先落地（收益最大、改动集中）。
2. 引入 Playwright `webServer` + globalSetup/Teardown 并删除旧 runner；随后更新 `package.json` scripts 与 CI 调用。
3. check-i18n 重写。
4. 引入 `crates/test-support` 并逐步迁移 Rust tests（优先 server/config/app-runtime）。
