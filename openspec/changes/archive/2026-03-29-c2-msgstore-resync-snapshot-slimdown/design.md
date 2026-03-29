## Context

`logs-store::MsgStore` 通过 `tokio::sync::broadcast` 向多个订阅者输出 raw / normalized entry 事件流。由于 broadcast 具备有界容量，慢订阅者会收到 `RecvError::Lagged(skipped)`。

当前实现的 lag 处理策略是：

1. 生成 retained history 的 snapshot（`raw_history_page(usize::MAX, None)` / `normalized_history_page(usize::MAX, None)`）
2. 将 snapshot 的每条 entry 再包装为 `LogEntryEvent::Replace` 推入 `pending: VecDeque<LogEntryEvent>`
3. 逐条从 pending pop 并输出

该策略在 retained history 较大或 lag 频繁时，会产生明显的瞬时内存/分配放大（snapshot + pending 双份），并导致 resync 时的 CPU/alloc 抖动。

## Goals / Non-Goals

**Goals:**

- lag resync 时避免对同一份 snapshot 进行“二次物化”（不再构建 Replace 事件队列副本）。
- 保持现有对外语义一致（事件顺序、Finished 行为、Replace 的 entry_index 覆盖语义）。
- 维持实现的可读性与测试可覆盖性（raw 与 normalized 路径对称）。

**Non-Goals:**

- 不改变 broadcast 容量与 retained history 的预算/驱逐策略。
- 不引入新的事件类型（例如 Reset/Invalidate），不修改前端协议。
- 不在本变更中做更激进的结构调整（例如拆锁、ring-buffer 等）。

## Decisions

### 1) Resync 以“snapshot + cursor”状态替代 `pending VecDeque`

**Decision:** 在 stream unfold 的状态中增加一个可选的 resync 状态：

- `snapshot: Vec<LogEntrySnapshot>`
- `pos: usize`
- `emit_finished: bool`（snapshot 输出完成后是否需要补发 Finished）

每次 poll/迭代优先从 resync snapshot 中按 `pos` 产出一个 `Replace` 事件；当 snapshot 耗尽后（若需要）再产出 `Finished`；随后回到正常 rx.recv()。

**Why:** 该方式只保留一份 snapshot（Vec）并按需生成 Replace 事件，避免 snapshot→pending 的二次分配与复制。

**Alternative considered:** 继续使用 pending，但改为分批 push（chunk）以降低峰值；仍会存在额外队列副本，收益更小且状态更复杂。

### 2) 保持 snapshot 获取方式不变（复用现有 `*_history_page`）

**Decision:** 继续使用 `raw_history_page/normalized_history_page` 获取全量 snapshot（受预算限制），不新增新的 snapshot API。

**Why:** 变更聚焦在避免二次物化；保持 snapshot 生成逻辑不变能降低改动风险，并复用已有元数据/日志输出。

## Risks / Trade-offs

- [Risk] resync 与 live event 交错时序错误 → **Mitigation**：resync 状态优先输出直到耗尽，再回到 rx；并用现有 lag/resync 测试覆盖 raw/normalized 两条路径。
- [Risk] Finished 重放逻辑遗漏/重复 → **Mitigation**：在 snapshot 时读取 `inner.finished`，将其作为 resync 状态的一部分，确保 Finished 只在需要时发出一次。

## Migration Plan

- 无需数据迁移；仅代码实现替换。

## Open Questions

- （无）
