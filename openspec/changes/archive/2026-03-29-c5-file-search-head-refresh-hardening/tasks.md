## 1. Cache budgets 与启动日志

- [x] 1.1 调整 `file_search_head_check_ttl` 默认值到 1–2s 区间，并确保 `VK_FILE_SEARCH_HEAD_CHECK_TTL_SECS` 仍可覆盖（验证：`cargo test -p config`）
- [x] 1.2 新增 truncated repo 重建节流 budget（例如 `VK_FILE_SEARCH_TRUNCATED_REBUILD_MIN_INTERVAL_SECS`）并在 runtime 启动日志输出（验证：`cargo test -p app-runtime`）

## 2. GitService HEAD OID 轻量解析（含 worktree + packed-refs）

- [x] 2.1 在 `GitService` 增加 `get_head_oid_fast()`：优先通过解析 `.git`/`gitdir`/`commondir`/`HEAD`/`refs/*`/`packed-refs` 获取 OID，失败才回退到 Git CLI（验证：新增单测）
- [x] 2.2 补充测试覆盖：普通 repo、worktree（`.git` 为 `gitdir:` 文件）、packed-refs（`git pack-refs --all --prune`）下 `get_head_oid_fast()` 与 `git rev-parse HEAD` 一致（验证：`cargo test -p repos git::tests::...`）

## 3. FileSearchCache 刷新策略硬化

- [x] 3.1 head check worker 使用 `get_head_oid_fast()`，避免周期性 spawn git（验证：`cargo test -p repos`）
- [x] 3.2 对 `index_truncated` 仓库应用最小重建间隔；补充单测验证“未到间隔不 enqueue build，到间隔可 enqueue”（验证：`cargo test -p repos file_search_cache::tests::...`）

## 4. 验收与归档

- [x] 4.1 运行 `just qa` 与 `just openspec-check`，失败则自修复并重跑直到通过
