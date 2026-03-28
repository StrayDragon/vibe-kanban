## Context

当前日志系统核心数据结构 `logs_store::MsgStore` 负责：
- 保存 `LogMsg` 历史（供新订阅者 replay）。
- 通过 tokio broadcast 提供实时订阅（SSE/WS/内部 normalizer）。
- 将 stdout/stderr 与规范化后的 JSON Patch（`LogMsg::JsonPatch`）统一进同一条消息流。
- 同时维护 raw/normalized “entry-indexed” 流（Append/Replace/Finished），用于 UI 的增量渲染与分页回补。

现状的主要性能/内存问题集中在：
- 字节预算估算使用 `serde_json::to_string(...).len()`（patch/entry-json 都会触发额外序列化 + 分配）。
- broadcast 通道容量默认 10k，且存在冗余通道（同 payload 多份保存），导致常驻内存不稳定。
- `MsgStore::push` 在持锁期间做 patch 解析/提取与字符串 clone，增加锁竞争。
- lagged 处理不一致：entry-indexed 流可 resync，而 `LogMsg` 流在 lagged 时可能静默缺失（需要统一为可恢复）。

约束：
- 不改变对外 HTTP/SSE/WS payload 的字段结构与语义（仅优化内部实现与可配置项）。
- 预算估算必须“保守”（宁可略高估，也不能低估导致预算失效）。

## Goals / Non-Goals

**Goals:**
- 移除热路径上的 `serde_json::to_string(...).len()`，把预算估算改为无堆分配、线性扫描的近似/精确长度估算。
- broadcast 容量可配置并下调默认值，在 lagged 时可 resync（避免静默缺失）。
- 减少 `MsgStore::push` 的锁内工作与 clone，降低 CPU 与锁竞争。

**Non-Goals:**
- 不引入新的持久化格式/表结构；不重做 DB backfill 逻辑。
- 不在本变更中对 UI 渲染策略做大改（只保证现有端到端行为继续通过）。

## Decisions

### 1) JSON 长度估算：用“估算器”替代序列化

实现一个无堆分配的 JSON 长度估算器（核心：对 `serde_json::Value` 与 `json_patch::Patch` 进行结构化遍历）：
- `Value`：递归计算 `null/bool/number/string/array/object` 的 JSON 字符串长度；
  - string 使用字节扫描计算 escaping 长度（`"`, `\\`, control chars 等）；
  - number 使用无分配长度计算（整数 digit count；浮点使用 `ryu` buffer 计算）。
- `Patch`：按 JSON Patch 规范的序列化形态（`[{op,path,from?,value?}, ...]`）叠加字段长度与 `Value` 估算。

策略：以“尽量接近 serde_json 输出长度”为目标，但对不确定分支保持保守（>= 实际长度），避免低估导致预算失效。

### 2) broadcast 容量：可配置 + 默认下调

新增环境变量 `VK_LOG_BROADCAST_CAPACITY`（用于 MsgStore 内各 broadcast 通道容量），默认显著小于 10k（例如 1024）。
理由：
- broadcast 的 ring buffer 以“条数”计，不以“字节”计；容量过大会让大 payload 造成不可控内存。
- 系统已经具备 lagged 的 resync 能力（entry-indexed 流已有），补齐 `LogMsg` 流后即可安全下调。

### 3) 统一到 sequenced 流：减少冗余通道与重复保存

`MsgStore` 内部以 `SequencedLogMsg` 作为唯一实时消息流：
- `LogMsg` 流（历史 + 实时）从 sequenced 流派生（过滤/映射到 `LogMsg`）。
- lagged 时基于 seq 做“从历史补齐”的 resync，并用 `seq` 去重，避免重复处理。

### 4) 降低锁内开销

在进入 `inner.write()` 之前尽量完成：
- `JsonPatch` 的 normalized-update 提取（会产生 `Value` clone）；
- stdout/stderr 的字符串 clone（用于 raw entry）。

锁内只做：
- 递增 seq；
- 写入 history；
- 写入 raw/normalized entry 容器并产出事件；
- 更新 finished 标记。

## Risks / Trade-offs

- [预算估算偏高] → 可能更早 eviction，造成历史更短。Mitigation：测试覆盖典型 payload，确保估算接近真实长度；只在极少场景保守加成。
- [budget/lag 配置不当] → broadcast capacity 过小导致频繁 lagged。Mitigation：默认值选择保守（但远小于 10k）；lagged 统一走 resync 且带指标/日志。
- [resync 的重复/漏消息] → 如果 seq 去重实现不正确会导致 normalizer 重复处理或遗漏。Mitigation：新增单测覆盖 lagged 场景与去重语义。

## Migration Plan

- 本变更为纯运行时行为优化，无需 DB migration。
- 发布后可通过 `VK_LOG_BROADCAST_CAPACITY` 回退到较大值以降低 lagged 概率（保留兼容）。
- 若出现意外 eviction/lagged 增多，可临时调高容量或调高 `VK_LOG_HISTORY_MAX_BYTES/ENTRIES`。

## Open Questions

- 默认 `VK_LOG_BROADCAST_CAPACITY` 选取是否需要与 `VK_LOG_HISTORY_MAX_ENTRIES` 建议联动（例如取 `min(1024, history_max_entries)`）？
- 是否需要把不同通道（sequenced/raw/normalized）的容量拆分为独立 env（当前先统一以简化配置）？
