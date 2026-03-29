## Context

现状（已完成 c4）：`FileSearchCache::search()` 的 cache hit 不再同步调用 Git 校验 HEAD；改为后台异步 head check + TTL 门控，发现 HEAD OID 变化后 enqueue 索引重建。

残留问题：
- **后台成本**：head check 仍通过 `git rev-parse HEAD` 获取 OID，在活跃输入场景下会形成“周期性 spawn Git 进程”的低效开销。
- **旧索引窗口**：为避免放大，head check TTL 通常不会太小，HEAD 切换后存在短暂旧索引窗口。
- **大仓库抖动**：对 `index_truncated` 的仓库，任何 HEAD 变化都可能触发昂贵的全量重建，频繁切分支/操作会导致抖动。

约束：
- 不引入旧写法兼容层；改动聚焦于 `repos` + `config`。
- 覆盖 worktree 场景（`.git` 为 `gitdir:` 文件 + `commondir`）。
- 覆盖 packed-refs（refs 不一定以 loose file 形式存在）。

## Goals / Non-Goals

**Goals:**
- HEAD OID 解析优先走轻量文件读取（`.git` / `gitdir` / `commondir` / `HEAD` / `refs/*` / `packed-refs`），尽量不 spawn Git。
- 默认 head check TTL 更小（1–2s），但不把成本放大到“每秒 spawn Git”。
- 对 `index_truncated` 仓库引入重建节流（最小重建间隔可配置），避免抖动。

**Non-Goals:**
- 不实现“按变更集增量更新索引”（只做刷新策略与触发策略）。
- 不改变索引构建结果与搜索语义（只优化刷新/成本/稳定性）。
- 不强依赖文件 watcher（保持纯轮询/按需刷新即可工作）。

## Decisions

1) **新增 HEAD OID 轻量解析器（带回退）**
- **Decision**: 在 `GitService` 中新增 `get_head_oid_fast()`（或等价命名）：先尝试文件解析拿到 OID；无法解析时回退到 `GitCli rev-parse HEAD`。
- **Rationale**: Git CLI 进程启动成本高；而 HEAD/OID 信息本质上可由少量 git 元数据文件解析得到。
- **Alternatives**:
  - 始终 `git rev-parse`：简单但成本高。
  - 只 stat 不读取：可能因时间戳分辨率导致漏检；且仍需最终拿到 OID。

2) **worktree `gitdir/commondir` 解析策略**
- **Decision**: 解析 `<repo>/.git`：
  - 若为目录：`git_dir = <repo>/.git`，`common_dir = git_dir`
  - 若为文件：读取 `gitdir: ...` 得到 worktree gitdir；若 `commondir` 文件存在，则 `common_dir = git_dir + commondir`
- **Rationale**: 这是 Git worktree 的标准布局，保证 HEAD / refs / packed-refs 的寻址正确。

3) **refs 与 packed-refs 的 OID 解析**
- **Decision**: 读取 `git_dir/HEAD`：
  - detached：HEAD 直接包含 OID
  - symbolic ref：优先读 `common_dir/<ref>`（loose ref），若不存在则从 `common_dir/packed-refs` 线性扫描匹配 ref
- **Rationale**: loose refs 为最常见；packed-refs 作为回退覆盖边界情况。

4) **FileSearchCache 使用轻量解析 + 大仓库重建节流**
- **Decision**:
  - head check worker 统一使用 `get_head_oid_fast()` 获取 OID（仅在必要时 spawn Git）。
  - 对 `cached.index_truncated == true` 的仓库，若距离上次 `build_ts` 过短，则跳过本次 enqueue build（最小间隔来自 cache budgets）。
- **Rationale**: 大仓库全量重建成本高，且 truncated 已意味着“可接受部分结果”；优先稳定与避免抖动。

5) **更短 TTL 的默认值**
- **Decision**: 将 `file_search_head_check_ttl` 默认值下调到 1–2 秒范围，并保留 env 覆盖。
- **Rationale**: 缩短 HEAD 切换后的旧索引窗口；在轻量解析下不会导致频繁 spawn Git。

## Risks / Trade-offs

- **[解析覆盖不足]** 非标准 Git 布局导致回退到 Git CLI，收益不稳定 → **Mitigation**: 强测试覆盖（普通 repo / worktree / packed-refs），并保持回退路径可靠。
- **[packed-refs 扫描成本]** packed-refs 可能较大 → **Mitigation**: 仅在 loose ref 缺失时扫描；必要时可在后续引入 mtime 缓存（本次不做复杂化）。
- **[节流导致更长更新窗口]** truncated repo 可能更久不重建 → **Mitigation**: 仅对 truncated 启用更大间隔，且 env 可调小；不影响小/中仓库。

## Migration Plan

- 新增/调整 cache budgets（纯 env，可选），默认值向后兼容且不需要数据迁移。
- 回滚策略：将 TTL 调大或禁用节流；HEAD OID 解析可退回到 Git CLI 路径（仍可工作）。

## Open Questions

- truncated repo 的默认最小重建间隔是否需要按平台（macOS/Linux/Windows）微调？
- packed-refs 扫描是否需要在后续引入“mtime + ref->oid 缓存”来进一步降低 I/O？
