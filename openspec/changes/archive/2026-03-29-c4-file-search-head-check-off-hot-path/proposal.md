## Why

当前 `FileSearchCache::search()` 在 cache hit 时仍会同步调用 `GitService::get_head_info()`（`git symbolic-ref` + `git rev-parse`）。文件搜索通常被前端按“每次键入/每次请求”触发，这会在热路径上频繁 spawn Git 进程，导致不必要的 CPU/内存占用与延迟抖动。

## What Changes

- `FileSearchCache::search()` 在 cache hit 时不再同步调用 Git 获取 HEAD 信息；直接用已缓存的索引返回结果。
- 引入“异步 + TTL 门控”的 HEAD 校验/刷新：在后台低频检查 HEAD OID，发现变化后再触发索引重建（复用现有 build queue）。
- HEAD 校验只需要 OID：使用只读的 `git rev-parse HEAD`（或等价封装）替代 `get_head_info()`，避免多余的 `symbolic-ref`。
- 增加测试覆盖：验证 cache hit 不再因 HEAD 不匹配而同步 miss；并验证 HEAD 变化会在后台触发重建与 cache 更新。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `file-search-index`: 增加性能护栏——cache hit 的搜索热路径不应同步执行 Git 进程；HEAD 变化检测应为异步、TTL 门控的刷新机制，确保最终一致并避免请求放大。

## Impact

- Backend: `crates/repos/src/file_search_cache.rs`（热路径命中逻辑、后台 HEAD 校验队列/状态、触发重建）
- Backend: `crates/repos/src/git/mod.rs`（提供只读获取 HEAD OID 的轻量封装，供文件搜索使用）
- Tests: `crates/repos/src/file_search_cache.rs`（新增异步刷新/不阻塞热路径的验证）

## Goals

- 显著降低文件搜索（按键触发）的 CPU/内存开销与延迟抖动，避免每次请求 spawn Git 进程。
- 保持缓存索引的最终一致：HEAD 变化能在合理时间窗口内触发重建并更新缓存。

## Non-goals

- 不改动索引构建策略（walker/check-ignore/ranking）的整体结构与结果排序逻辑。
- 不重新启用或扩大 watcher 覆盖范围（本变更仅聚焦 HEAD 校验从热路径移出）。
- 不改变 API 返回结构与前端交互协议。

## Risks

- 在 HEAD 切换后的短时间窗口内可能返回旧索引结果（通过 TTL 门控与后台刷新缩短窗口）。
- 后台队列在多 repo 场景下可能堆积（通过有界队列 + coalesce/去重避免放大）。

## Verification

- 单元测试：cache hit 在 HEAD 已变化时仍返回结果（不同步 miss）；并在后台刷新后更新 `head_sha`。
- 运行：`cargo test -p repos`、`just qa`、`just openspec-check`。
