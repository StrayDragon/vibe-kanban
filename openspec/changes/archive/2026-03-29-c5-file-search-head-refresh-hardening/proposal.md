## Why

当前文件搜索索引的 HEAD 刷新机制虽然已从热路径移出，但仍存在 3 个残留风险：1）HEAD 切换后存在短暂旧索引窗口；2）后台仍会周期性 spawn `git rev-parse` 带来低效 CPU/内存开销；3）对大仓库（索引截断）来说，HEAD 变化可能触发昂贵重建并造成抖动。

## What Changes

- 将后台 HEAD OID 解析从 `git rev-parse` 优先改为“基于 `.git`/`gitdir`/`commondir`/`packed-refs` 的轻量解析”，仅在无法解析时才回退到 Git CLI。
- 将默认 HEAD 校验 TTL 调整到更小（建议 1–2s），但通过轻量解析避免“更高频=更高成本”的放大。
- 对索引截断（大仓库）引入重建频率上限：HEAD 变化触发重建前，必须满足最小间隔（可配置），避免反复 checkout/rebase 时的重建抖动。

## Capabilities

### New Capabilities
- （无）

### Modified Capabilities
- `file-search-index`: 增加更严格的性能/稳定性护栏——HEAD 校验应尽可能不 spawn Git；大仓库重建需节流；并在更短 TTL 下保持可接受成本。

## Impact

- Backend: `crates/repos/src/git/mod.rs`（新增/扩展 HEAD OID 轻量解析能力，覆盖 worktree `gitdir/commondir` 与 `packed-refs`）
- Backend: `crates/repos/src/file_search_cache.rs`（head check worker 使用轻量解析；对 truncated index 引入重建节流）
- Backend: `crates/config/src/cache_budget.rs` + `crates/app-runtime/src/lib.rs`（新增/调整相关 cache budgets 并输出启动日志）
- Tests: `crates/repos/src/git/mod.rs` 与 `crates/repos/src/file_search_cache.rs`（覆盖 packed-refs + worktree 场景，确保切换后行为一致）

## Goals

- 在不回退到同步 HEAD 校验的前提下，显著降低后台 HEAD 校验的 CPU/内存开销（尽量不 spawn Git）。
- 在更短的 HEAD 校验 TTL 下仍保持低成本，缩短 HEAD 切换后的旧索引窗口。
- 对大仓库（索引截断）避免 HEAD 变化带来的重建抖动，提升整体稳定性。

## Non-goals

- 不实现增量索引（按变更集更新索引）；仅做刷新与重建触发策略的优化。
- 不改变文件索引构建内容与排序/搜索结果语义（避免引入行为差异）。
- 不引入全量 repo watcher 作为必需依赖（保持机制可在无 watcher 情况下工作）。

## Risks

- 轻量解析覆盖面不足导致回退到 Git CLI，性能收益不稳定 → 通过 worktree + packed-refs 的测试覆盖与渐进回退路径降低风险。
- 过强节流可能延长大仓库索引更新窗口 → 仅对 `index_truncated` 启用更长间隔，并提供 env 可调。

## Verification

- 单测：HEAD OID 轻量解析在普通 repo / worktree / packed-refs 下与 `git rev-parse HEAD` 一致。
- 单测：对 truncated index 的 repo，HEAD 连续变化不会触发高频重建（满足最小间隔）。
- 运行：`cargo test -p repos`、`just qa`、`just openspec-check`。
