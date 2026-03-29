## Tasks

- [x] 1. GitCli：引入可复用的临时 index（prepared temp index）
  - [x] 抽取 staging 逻辑为内部结构体（`TempIndex`/`PreparedTempIndex`），复用一次 `git add` 结果
  - [x] 在同一 prepared index 上支持运行 name-status 与 numstat（保持现有参数与 pathspec 过滤语义）
  - [x] 增加测试级计数/钩子（仅测试用）用于验证“单次 staging”

- [x] 2. GitService：基于一次 prepared index 生成 worktree diff plan
  - [x] 新增 `WorktreeDiffPlan`（包含 status entries + numstat index + total_bytes 等必要信息）
  - [x] `get_worktree_diff_summary` 与 `get_diffs(DiffTarget::Worktree)` 改为复用 plan，避免重复 staging

- [x] 3. Server：attempt changes/patch 复用同一次 plan
  - [x] `/api/task-attempts/:id/changes` 用 plan 生成 summary + files（避免额外 get_diffs 调用）
  - [x] `/api/task-attempts/:id/patch`：guard 评估与 patch 生成复用同一次 plan（避免二次 staging）

- [x] 4. 测试与验收
  - [x] `cargo test -p repos`
  - [x] `cargo test -p server`
  - [x] `just qa`
  - [x] `just openspec-check`

- [x] 5. 归档与提交
  - [x] `openspec archive -y c3-worktree-diff-index-reuse`
  - [x] 创建 commit：`refactor: worktree-diff-index-reuse`
