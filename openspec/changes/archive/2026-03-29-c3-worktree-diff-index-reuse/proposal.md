## Why

当前 `/api/task-attempts/:attemptId/changes` 与 `/api/task-attempts/:attemptId/patch` 会在同一次请求中重复做“worktree diff 预处理”（temp index + `git add` + `git diff`），导致 CPU/IO 放大，尤其在包含大量 untracked/rename 的仓库里会显著拖慢响应并增加系统负载。

我们需要把 worktree diff 的准备与统计/paths 生成合并成“单次最小必要工作”，减少重复 git 进程与 staging 成本，从而降低整体 CPU/内存占用并提升交互流畅度。

## What Changes

- 在 `repos` 的 Git CLI diff 实现中引入“可复用的临时 index（prepared temp index）”，同一次 diff 计算复用一次 staging 结果。
- 为 attempt changes/patch 提供单次 worktree diff 计划（status + numstat）能力，避免在同一次 API 请求中重复 staging。
- 增加测试覆盖：确保 refactor 前后 diff summary/paths/patch 行为一致，并为“单次 staging”引入可验证的护栏（测试级别）。

## Capabilities

### New Capabilities

- （无）

### Modified Capabilities

- `diff-preview-guardrails`: 强化 diff 预览链路的性能护栏：worktree diff summary 与 paths/patch 生成应复用同一次临时 index/staging 结果，避免重复的 `git add`/`git diff` 预处理工作。

## Impact

- 影响 Rust 侧 git diff 代码路径：`crates/repos/src/git/cli.rs`、`crates/repos/src/git/mod.rs`；以及 attempt API handler：`crates/server/src/routes/task_attempts/handlers.rs`。
- 不改变 API response shape；属于内部性能重构。
- 验证：新增/更新单测（`cargo test -p repos`、`cargo test -p server`），并通过 `just qa` + `just openspec-check`。

