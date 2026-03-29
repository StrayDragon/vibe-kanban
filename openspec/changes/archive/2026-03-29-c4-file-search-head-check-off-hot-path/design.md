## Context

当前文件搜索走 `crates/repos/src/file_search_cache.rs::FileSearchCache::search()`。在 cache hit 时仍会同步调用 `GitService::get_head_info()` 来校验 `HEAD` 是否匹配缓存的 `head_sha`，这会在热路径上频繁 spawn Git CLI 进程（且每次至少两条命令：`symbolic-ref` + `rev-parse`）。

文件搜索通常是“按键触发”的高频操作（前端输入框联想/过滤），因此这一同步 HEAD 校验会显著放大 CPU/内存占用与尾延迟。

约束：
- 不引入 legacy 兼容层；保持改动聚焦且可验证。
- 继续使用 Git CLI（仓库已有 `GitCli` 统一封装）。
- 不依赖 watcher（当前 warm_most_active 中 watcher 仍被禁用，且大 repo 上 watcher 本身有噪声/成本）。

## Goals / Non-Goals

**Goals:**
- cache hit 的搜索请求不再同步执行 Git 命令；命中后直接返回结果。
- HEAD 变化检测改为后台异步刷新，并通过 TTL 门控避免高频请求放大。
- HEAD 校验最小化：只获取 OID（`rev-parse HEAD`），不做 `symbolic-ref`。

**Non-Goals:**
- 不改变索引构建与排名逻辑（walker/check-ignore/ranker 的输出行为保持一致）。
- 不新增“全 repo 文件 watcher”或扩大 watcher 覆盖（仅在需要时做轻量 HEAD 校验）。
- 不改变 API contract（仍由调用方处理 `CacheError::Miss` 并等待预热/重建）。

## Decisions

1) **把 HEAD 校验移出热路径（异步 worker）**
- **Decision**: `search()` 在 cache hit 时直接返回缓存结果，同时“按需”调度后台 HEAD 校验任务。
- **Rationale**: 热路径避免同步 I/O 与进程 spawn；即便后台校验失败，也不影响当前查询响应。
- **Alternatives**:
  - 同步 `rev-parse` 但加 TTL：仍会在请求线程里阻塞/抖动，且并发下更容易放大。
  - 读取 `.git/HEAD` + 解引用 ref：实现复杂且边界多（packed-refs/worktree/gitdir），风险大。

2) **按 repo 去重 + TTL 门控的调度策略**
- **Decision**: 以 `repo_path` 为 key 记录 `last_head_check`，在 TTL 内不重复调度；并用 `pending_head_checks` 去重队列入队。
- **Rationale**: 避免“每次键入都触发后台任务”的隐性放大；同时保证在持续活跃的 repo 上仍能周期性自愈。
- **Default TTL**: 新增 cache budget `file_search_head_check_ttl`，默认 5s（可通过 env 覆盖）。

3) **HEAD 校验只比较 OID，不关心 branch**
- **Decision**: 为 `GitService` 增加轻量 `get_head_oid()`（内部只做 `rev-parse HEAD`）。
- **Rationale**: 文件搜索缓存只需要判断“HEAD 是否变化”；branch 名称不参与决策，避免额外命令与字符串分配。

4) **触发重建复用现有 build queue**
- **Decision**: 后台 HEAD worker 发现 OID 变化时，仅负责 enqueue build（复用 `pending_builds` 去重逻辑）；真正构建仍由现有 `background_worker` 完成。
- **Rationale**: 维持单一构建入口，避免并发重建/状态分裂；同时便于测试验证重建发生。

## Risks / Trade-offs

- **[Stale window]** HEAD 切换后短时间内仍返回旧索引 → **Mitigation**: TTL 默认 5s；并在 repo 活跃时持续刷新，最终一致。
- **[Queue backlog]** 多 repo 同时活跃可能堆积 HEAD 校验 → **Mitigation**: 有界队列 + 去重（每 repo 仅 1 个 pending）。
- **[Git 不可用/错误]** 校验失败导致不触发重建 → **Mitigation**: 校验失败仅记录日志；下一次 TTL 到期可再次尝试；cache TTL 仍会兜底驱逐/重建。

## Migration Plan

- 无数据/配置迁移；只新增一个可选 env（cache budget），默认值保证行为可预测。
- 回滚策略：若出现异常（例如 stale window 不可接受），可恢复为同步校验或缩短 TTL（无需 schema 变更）。

## Open Questions

- TTL 默认值是否需要按平台区分（例如 Windows 上 Git 进程启动更慢，可能希望更大 TTL）？
- 是否需要对 truncated index 的 repo 进一步降低刷新频率（避免大 repo HEAD 变化时重建成本过高）？
