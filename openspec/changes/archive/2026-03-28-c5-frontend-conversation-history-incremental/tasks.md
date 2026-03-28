## 1. 增量派生与缓存

- [x] 1.1 为 conversation history 引入 per-process 派生缓存（entries/统计/游标等），并将更新范围收敛到“被触达的 execution process id”。
- [x] 1.2 统一 execution processes 的排序/过滤入口（输出 canonical `processesSorted` + `processesById`），移除重复 sort/filter 热点。
- [x] 1.3 为缓存引入上限与淘汰策略（LRU 或等价），避免长会话导致常驻内存增长。

## 2. Tests

- [x] 2.1 增加单测：大输入下局部更新不触发全量重算（用计数器/spy 断言或等价手段），并锁定派生结果顺序/内容一致。
- [x] 2.2 增加单测：缓存淘汰与加载 older history 的边界行为（loading 状态、truncated 标志）保持一致。

## 3. Verification

- [x] 3.1 Run `pnpm -C frontend run test` and `pnpm -C frontend run lint`.
- [x] 3.2 Run `just qa` and `just openspec-check`.
