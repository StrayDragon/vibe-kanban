## Context

worktree diff 计算在本项目中为了保证：

- sparse-checkout 语义正确
- untracked 文件也能出现在 diff/patch 中
- rename 检测尽量准确

因此在 `GitCli` 侧使用“临时 index（`GIT_INDEX_FILE`）+ `git read-tree HEAD` + `git add -A`（带 pathspec-from-file）+ `git diff --cached ...`”的策略来生成 status/numstat。

问题在于：同一条 API 链路里经常需要 **同时** 拿到：

- `DiffSummary`（file_count/additions/deletions/total_bytes）
- 变更文件列表（paths）
- 某些路径的 patch 内容（unified diff）

当前实现会为这些不同产物重复做 temp index + staging，导致 `git add`/`git diff` 被多次执行，CPU/IO 放大明显。

## Goals / Non-Goals

**Goals:**
- 同一次 worktree diff 计算复用一次 staging（prepared temp index），避免重复 `git add`。
- attempt changes/patch 链路不改变返回 JSON shape 与阻断（guardrail）语义。
- 增加测试覆盖，确保 diff 行为不回归，并能验证“单次 staging”的约束（测试级别）。

**Non-Goals:**
- 不改变 diff-preview-guardrails 的阈值策略与判断逻辑。
- 不引入新的缓存（跨请求缓存），本变更只做“单请求内去重/复用”。

## Decisions

1) **引入 PreparedTempIndex（GitCli 内部可复用 staging 结果）**
- 抽取出“创建临时 index + read-tree + 根据 worktree status 构造 pathspec + git add”的公共步骤，封装为一个持有 `TempDir` 的结构体（生命周期绑定到一次 diff 计算）。
- 在同一个 prepared index 上执行：
  - `git diff --cached -M --name-status -z <base>`
  - `git diff --cached --numstat -z <base>`
  从而实现一次 staging，多次读取。

2) **GitService 基于一次 prepared index 产出 diff plan**
- 新增内部 helper（或公共方法）一次性返回：
  - `Vec<StatusDiffEntry>`
  - `Vec<NumstatEntry>` / `NumstatIndex`
- `get_worktree_diff_summary` 与 `get_diffs`（worktree target）都改为基于该 plan，避免各自再创建临时 index。

3) **API handler 侧减少重复 diff 调用**
- `/changes`：避免 “summary + get_diffs(omit)” 的双调用；改为一次调用得到 summary 与 paths。
- `/patch`：保持 guardrail 语义不变；在可行范围内复用同一次 plan 来生成 patch（避免二次 staging）。

## Risks / Trade-offs

- [Risk] staged index 复用导致某些边界情况（奇异文件名、rename 检测）行为变化 → Mitigation：沿用现有 git 命令参数与 pathspec 构造逻辑；新增回归测试覆盖 rename/untracked 情况。
- [Risk] 临时 index 生命周期管理不当导致临时目录泄漏 → Mitigation：使用 `tempfile::TempDir` 持有并自动清理，且不跨请求缓存。
- [Trade-off] 引入少量结构体与代码重构 → 通过集中复用减少长期维护成本（避免重复实现 staging）。

