## Context

当前仓库的 Git 能力处于混合状态：

- `crates/repos/src/git/cli.rs` 已经承担了大量“会触碰 working tree、可能造成数据损失”的操作（worktree、rebase/merge、staging/commit 等），并在文件头解释了为什么更偏向 Git CLI 的语义与安全性。
- `crates/repos/src/git/mod.rs` 与 `crates/repos/src/worktree_manager.rs` 仍依赖 `git2`（libgit2）实现多类读操作、repo 初始化、签名/配置读取，以及 worktree ensure 的元数据检查/分支创建等。

这导致：

- 依赖链复杂（libgit2/OpenSSL），跨平台构建与运行失败点更多
- 同一能力存在两套实现边界，长期更容易出现行为漂移
- 难以把“Git CLI-only”作为明确的契约与可验证目标

本设计目标是在不改变用户可见行为的前提下，把所有 Git 行为统一收敛到 Git CLI，并在切换前补齐/固化测试覆盖以保证语义一致。

## Goals / Non-Goals

**Goals:**
- `crates/repos` 不再依赖 `git2`（生产依赖），所有 Git 能力通过 `GitCli` 实现
- worktree ensure / recreate 等流程完全 CLI 化，并继续满足现有 worktree 相关 spec 行为（幂等、缺失 metadata 目录可容忍等）
- 在移除 `git2` 前后均可运行的黑盒测试覆盖关键行为（避免“先删实现再补测试”）
- `git` 不可用时，提供清晰、可操作的诊断（并纳入 preflight）

**Non-Goals:**
- 不新增 Git 功能（例如新的远程集成、复杂认证策略）
- 不改变 API/MCP 工具的 contract（除非为了错误诊断更清晰而做兼容性允许的文案调整）
- 不引入长期双实现/兼容层（允许迁移过程中的短期过渡，但最终形态必须是 CLI-only）

## Decisions

### 1) 单一实现：Git CLI-only（移除 git2）

选择：把 `GitService`/`WorktreeManager` 中的 git2 依赖替换为 `GitCli` 调用，并最终从 crates 依赖中移除 `git2`。

备选：
- git2-only：不符合 `docs/decisions/git-strategy.md` 的稳定性/依赖面方向，也会继续放大 sparse-checkout、WSL 等风险。
- hybrid：短期可行但长期会持续漂移；本变更的目标是结束双实现。

### 2) 保持现有服务边界：外部接口不变、内部实现替换

选择：尽量保留 `repos::git::GitService` 对外 API 形态，内部实现切换为 CLI；避免上层（routes、execution、tasks）出现大范围重构。

说明：迁移完成后可考虑重命名/收敛模块结构（例如把 `git/mod.rs` 变为 CLI façade），但这不是本变更的必要条件。

### 3) diff/status/read path：以“可测试 + 可控解析”为优先

选择：
- 继续复用 `GitCli::diff_status`（临时 index + `git diff --cached --name-status`）作为 worktree diff 的“变更集合来源”（包含 untracked、支持 `-M` rename detection）。
- 对每个变更文件，使用 `git show <base>:<path>` + filesystem snapshot（或 `git show <commit>:<path>`）获取 old/new 内容；再按现有规则（大小阈值/二进制）生成 `utils_core::diff::Diff`。
- stats（additions/deletions）优先用 `git diff --numstat -z`，必要时回退到基于文本的 `compute_line_change_counts`。

备选：解析 `git diff --patch` 生成结构化 diff。可行但解析复杂度高；在我们当前 Diff DTO 仅需要“文件级 + 可选全文”时，不优先引入 patch 解析器。

### 4) repo init / identity / config：统一走 `git` 子命令

选择：
- repo 初始化：改用 `git init`（显式 main），必要时用 `git commit --allow-empty` 创建初始提交。
- commit identity fallback：使用 `git config --local user.name/user.email`（仅当缺失时写入）替代 `git2::Repository::config()`。
- 任何阻塞的 CLI 调用都必须放在 blocking 线程池中（或复用已有的 async-safe 调用点），避免卡住 Tokio runtime。

### 5) Cloud/clone 等远程能力：CLI 优先，认证策略最小化且不泄露

选择：为当前 `cloud` feature 迁移到 `git clone`/`git fetch`/`git push`，并优先依赖系统已有认证（credential helper、SSH agent）。若必须支持 token 注入，采用短生命周期、可审计、不会在日志中泄露的方式（例如 `GIT_ASKPASS` + 临时脚本/临时 helper），并在实现阶段评估跨平台可行性。

## Risks / Trade-offs

- [Git 版本/输出差异] → 优先使用 porcelain / `-z` 输出；对解析失败提供结构化错误并落入测试
- [性能开销（进程调用）] → 批量操作使用 `-z`/`--porcelain`，必要时引入 `git cat-file --batch`；同时确保在 blocking 线程池执行
- [认证与敏感信息] → 统一在 `GitCli` 里做 redaction（避免 token 出现在 error/log）；尽量依赖系统认证
- [测试变脆] → 黑盒测试尽量只断言稳定语义（状态/结果），避免断言易变的 stderr 文案

## Migration Plan

1. **测试基线补齐（在移除 git2 之前）**
   - 针对 worktree ensure、diff/status、sparse-checkout、安全保护（dirty/untracked）等补齐黑盒测试。
   - 新增测试要求：不依赖 `git2` 来读取对象（用 `git` CLI 校验 commit author、branch refs 等），确保测试能在 CLI-only 后继续使用。
2. **WorktreeManager CLI 化**
   - `git worktree list --porcelain` 替代 git2 的 worktree 注册检查
   - `git worktree add/remove/prune` 统一走 `GitCli`
   - attempt branch ensure 使用 `git show-ref`/`git branch`/`git update-ref` 等实现
3. **GitService read path CLI 化**
   - 分支列表、head info、merge-base、diff 生成等逐项迁移
   - 逐步删除 git2 相关辅助函数（signature/config/init 等）
4. **移除依赖与清理**
   - 从 `crates/repos` 及其他 crates 移除 `git2` 依赖
   - 评估并收敛 `openssl-sys`（确保仍满足剩余依赖链）
5. **验证闭环**
   - `cargo test --workspace`、`just qa`、`just openspec-check`

## Open Questions

- 远程 clone 的 token 注入（如仍需）采用哪种跨平台方案最稳妥？是否允许仅支持“系统认证”而不支持显式 token 参数？
- 对于超大 repo 的 diff 性能，是否需要在本变更内引入 `git cat-file --batch`/缓存机制，还是先以正确性为主？

