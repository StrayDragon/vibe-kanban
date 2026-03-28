## Why

`logs-perf-slimdown` 将日志热路径改为低分配并下调 broadcast buffer 默认容量（`VK_LOG_BROADCAST_CAPACITY=1024`）。这类改动的主要残留风险是：
- 在高吞吐/慢消费者场景更容易触发 lagged/resync，且需要更强的可观测性来定位与调参。
- JSON 长度估算器一旦发生低估，会削弱预算机制（隐性内存风险），需要更强的覆盖来防止回归。

因此需要补齐“可观测性 + 性质/随机覆盖 + 可控压力回归”，把风险转化为可验证、可诊断的行为。

## What Changes

- 为 `MsgStore` 的 lagged/resync 路径补齐更结构化的诊断输出（日志字段、关键元数据），并降低噪声/误判风险。
- 为 `approx_json_value_len` / `approx_json_patch_len` 增加性质测试（property-based），覆盖更广泛的随机 `Value` / `Patch` 组合，确保长度估算不低估且与 `serde_json::to_string(...).len()` 一致。
- 增加针对 log streaming 的压力/慢消费者回归测试（多订阅者、不同 lag window、eviction 边界），确保 resync 语义稳定且不出现重复/漏消息。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `execution-logs`: 明确 lagged/resync 的可观测性要求（例如 lag beyond retained window 必须可被日志明确诊断），便于线上调参与排障。

## Impact

- Rust crates:
  - `crates/logs-protocol`（性质测试/覆盖增强）
  - `crates/logs-store`（lagged/resync 诊断日志、压力回归测试）
- CI/QA：`cargo test --workspace` 运行时间可能小幅增加（需控制随机测试 case 数与最大深度）。

## Goals

- 让 lagged/resync 行为“可观测、可解释、可调参”（避免只靠猜测容量/历史预算）。
- 通过性质测试把 JSON 长度估算器的低估风险降到可接受水平（尽早在 CI 捕获）。
- 覆盖慢消费者/eviction 边界，避免高吞吐下的重复/漏消息回归。

## Non-goals

- 不引入新的 metrics/telemetry 系统（保持依赖面最小）；以结构化 `tracing` 日志与测试为主。
- 不改变对外 API/WS/SSE payload 格式与语义（仅补齐诊断与测试保障）。

## Risks

- 性质测试若参数过大可能拖慢 `just qa`：需要限制递归深度、集合大小与 case 数。
- 诊断日志若过于频繁可能产生噪声：需要控制日志级别与输出内容（必要时做简单的节流/降噪）。

## Verification

- `cargo test -p logs-protocol -p logs-store`
- `cargo test --workspace`
- `just qa`
- `just openspec-check`
