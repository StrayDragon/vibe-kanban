## Why

目前仓库同时使用了 Git CLI 与 `git2`（libgit2）两套实现：工作树相关的“破坏性/易踩坑操作”已经倾向走 CLI，但仍有大量读操作与 worktree ensure 逻辑依赖 `git2`。这带来：

- 依赖面与构建复杂度偏高（libgit2/openssl 链路，跨平台更容易踩坑）
- 双实现边界容易漂移，长期维护成本与行为一致性风险上升

我们已经在 `docs/decisions/git-strategy.md` 明确建议长期路线为 **Git CLI-only**，因此需要把该决策落到可执行的 spec + 任务闭环，并在切换前补齐行为回归测试覆盖，确保切换后仍能通过现有验证。

## What Changes

- 将 `crates/repos` 内所有 Git 能力收敛到 `GitCli`：读写/图查询/状态/差异/分支/工作树管理等均不再依赖 `git2`
- 迁移 `crates/repos/src/worktree_manager.rs` 的 worktree ensure / recreate 流程为 Git CLI 实现（`git worktree ...` + `git worktree list --porcelain`）
- 补齐与固化“切换前的行为基线”测试：覆盖当前依赖 libgit2 的关键路径与边界条件（worktree ensure、diff/numstat、branch 列表、sparse-checkout、dirty worktree 保护、错误分类等）
- 移除 `git2` crate 依赖链，并在可行范围内进一步减少/移除 `openssl-sys`（以 CI/发行形态稳定为目标）
- **BREAKING（内部）**：仓库内不再提供 libgit2 相关实现路径；Git 操作统一通过外部 `git` 可执行文件完成

## Capabilities

### New Capabilities
- `git-strategy`: 规定 Git 能力统一使用 Git CLI 的契约（依赖、错误语义、实现边界与迁移约束）

### Modified Capabilities
- `cli-dependency-preflight`: CLI 预检需要覆盖 `git` 可执行文件可用性，并对 Git 相关能力给出清晰诊断

## Goals

- 单一实现：所有 Git 能力走 Git CLI，避免长期双实现
- 可验证：在移除 `git2` 之前先把关键行为写成测试，保证切换后仍能通过
- 依赖收敛：减少 libgit2/OpenSSL 带来的系统依赖与构建失败点

## Non-goals

- 不在本变更中新增产品功能或改变用户可见的 Git 行为（目标是保持语义一致）
- 不在本变更中引入新的 Git 认证/账号体系（如需网络/鉴权能力，优先沿用 CLI 既有机制并保证敏感信息不泄露）

## Risks

- Git CLI 输出解析与跨版本差异可能引入边缘行为差异
- 进程调用开销增加（需确保关键路径不会阻塞 async runtime，必要时放到 blocking 线程池）
- 某些 git2 便利 API 迁移到 CLI 后实现复杂度上升（需要更严格的测试与错误分类）

## Verification

- `cargo test --workspace`
- `just qa`
- `just openspec-check`
- 针对 Git 相关测试的定向回归（例如 `cargo test -p repos`）

## Impact

- Rust crates：`crates/repos`（核心）、`crates/execution`/`crates/app-runtime`（间接依赖清理）、相关 route/handlers（调用链不变但实现后端变化）
- 依赖：移除 `git2`，收敛 `openssl-sys` 的使用范围
- 文档/规范：新增 `git-strategy` spec；更新 `cli-dependency-preflight` spec

