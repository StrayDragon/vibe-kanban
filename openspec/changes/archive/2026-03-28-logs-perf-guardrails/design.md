## Context

`logs-perf-slimdown` 已将日志链路的“字节预算”估算从热路径 `serde_json::to_string(...).len()` 替换为无堆分配的结构化遍历，并将 `MsgStore` 的 broadcast buffer 默认容量下调为 `1024`，同时为 `LogMsg` 流补齐 lagged resync。

这类改动的主要风险来自“高吞吐 + 慢消费者 + eviction 边界”：
- broadcast lagged 更频繁时，resync 是否会重复/漏消息、是否会退化到不可理解的行为；
- 预算估算器在极端 JSON 值上是否存在低估（CI 之外偶发）；
- 线上出现问题时，日志输出是否足够定位（lag 窗口、history window、跳过条数、起止 seq/index 等）。

本变更的定位是：**不改变对外协议语义** 的前提下，用可观测性与回归测试把上述风险封装成“可验证、可诊断”的闭环。

## Goals / Non-Goals

**Goals:**
- 为 lagged/resync 增加更结构化、可检索的诊断日志，便于线上调参（capacity/history budget）与定位慢消费者。
- 为 JSON 长度估算器补齐性质测试（property-based），覆盖更广泛的随机输入，降低低估回归概率。
- 增加可控的压力/慢消费者回归测试，覆盖：
  - 多订阅者同时 lagged
  - lag within retained window（可回放）
  - lag beyond retained window（显式退化 + 可观测）

**Non-Goals:**
- 不引入完整 metrics/telemetry 体系（Prometheus/OpenTelemetry 等），避免扩大依赖面与运维复杂度。
- 不改变对外 WS/SSE/HTTP payload 的结构与字段（仅补齐内部日志与测试保障）。

## Decisions

1) **性质测试选型：使用 `proptest`（dev-dependency）**
   - 目的：用 `prop_recursive` 生成受控深度/大小的 `serde_json::Value`（含字符串转义、嵌套对象/数组、数字），并与 `serde_json::to_string(...).len()` 做等价断言。
   - Patch 部分同理：生成“保证可解析”的 JSON Patch 形态（`op/path/from/value`），反序列化成 `json_patch::Patch` 后验证长度等价。
   - Alternative：自写随机生成（rand）或引入 `cargo-fuzz`。前者覆盖与 shrink 能力较弱；后者对 CI 与本仓库工作流侵入较大。

2) **压力测试策略：保持默认参与 CI 的测试“快且确定”**
   - 以 `tokio::test` 驱动，限制消息数量、订阅者数量与超时，确保 `cargo test --workspace` 可接受。
   - 对“更重”的压力场景（例如 10^5 消息、长时间跑）如有必要，使用 `#[ignore]` 作为手动/定期运行项，而不是阻塞 `just qa`。

3) **可观测性：统一使用结构化 `tracing` 字段**
   - lagged/resync 相关日志输出必须包含关键上下文（例如 skipped/last_seq/min_seq/max_seq 或 entry_index 范围）。
   - 对“lag beyond retained window”的退化路径必须输出可诊断字段，避免只看到“resyncing”但无法判断是否丢窗。
   - Alternative：增加新的 API/endpoint 暴露 counters；暂不做，优先用最小侵入方案完成闭环。

## Risks / Trade-offs

- [性质测试拖慢 CI] → 限制最大深度、集合大小与 case 数；避免生成超大字符串；必要时拆分为少量关键性质断言。
- [诊断日志噪声] → 仅在 lagged/resync 发生时输出；字段保持最小充分；如仍噪声过大再做节流（本变更优先不引入复杂节流状态）。
- [压力测试偶发超时导致不稳定] → 使用 `tokio::time::timeout`，并选择稳定的并发/容量参数；必要时在 CI 环境下调低消息量。

## Migration Plan

- 无需 DB migration、无用户配置迁移。
- 发布/回滚不需要额外步骤；本变更主要提升诊断与测试覆盖。

## Open Questions

- 是否需要为 lagged/resync 增加更明确的“可机器解析”字段规范（例如统一 event name / target）以便后续接入 metrics？
- 对超大字符串/深嵌套 JSON 的性质测试边界值（深度/元素数）应以 CI 资源预算为准，后续可按实际耗时微调。
