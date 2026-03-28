## 1. 补齐 Git 行为测试基线（切换前）

- [x] 1.1 盘点当前 `git2` 依赖点与对应行为（`crates/repos/src/git/mod.rs`、`crates/repos/src/worktree_manager.rs`）
- [x] 1.2 新增/补齐黑盒测试覆盖：worktree ensure（幂等、缺失 `.git/worktrees/`）、attempt branch recreate、dirty/untracked 保护、sparse-checkout 语义、diff/status/numstat（包含 untracked）
- [x] 1.3 将关键断言改为 Git CLI 验证（避免测试依赖 `git2` 读取对象），确保测试在 CLI-only 后仍可运行

Verification:
- `cargo test -p repos`

## 2. 扩展 `GitCli` 覆盖面（为 CLI-only 迁移提供原语）

- [x] 2.1 增加缺失的只读原语（例如：`rev-parse`/`merge-base`/`for-each-ref`/`show`/`cat-file` 等）并统一使用 `--porcelain`/`-z` 输出
- [x] 2.2 增加缺失的写入原语（例如：`init`/`branch`/`update-ref`/`clone`），并确保错误分类与敏感信息 redaction 一致
- [x] 2.3 为新增解析/封装逻辑补单测（输出解析、错误分类、redaction）

Verification:
- `cargo test -p repos git_`

## 3. WorktreeManager 迁移到 Git CLI

- [x] 3.1 用 `git worktree list --porcelain` 替代 git2 的 worktree 注册检查逻辑
- [x] 3.2 用 `GitCli` 实现 worktree recreate/cleanup/create 流程（`worktree add/remove/prune`）
- [x] 3.3 attempt branch ensure 逻辑 CLI 化（不存在则从 target/base 创建），并保持并发锁/幂等语义不变

Verification:
- `cargo test -p repos worktree_manager_ensure`

## 4. GitService 读路径与初始化流程 CLI 化

- [x] 4.1 `initialize_repo_with_main_branch` 等初始化/签名/身份逻辑迁移到 CLI（避免写入用户 git config；优先用进程级 env 注入身份）
- [x] 4.2 分支/HEAD/提交图查询相关 API 改为 CLI（`get_head_info`、`get_all_branches`、`get_merge_base` 等）
- [x] 4.3 diff 生成完全去除 git2：基于 `diff_status` + `git show` + filesystem snapshot 产出 `Diff` DTO（含 `content_omitted` 与 additions/deletions）

Verification:
- `cargo test -p repos git_workflow`

## 5. 移除 `git2` 依赖链（以及可行的 OpenSSL 收敛）

- [x] 5.1 从 `crates/repos` 与其它仍引用的 crates 移除 `git2` 依赖与代码路径
- [x] 5.2 清理 `openssl-sys`：仅在确有需要的链路保留（若 git2 被完全移除，应评估能否从 workspace deps 移除）
- [x] 5.3 更新 crate-boundaries/CI 脚本，确保 CLI-only 不引入新的跨层依赖

Verification:
- `cargo test --workspace`
- `./scripts/check-crate-boundaries.sh`

## 6. 更新 CLI 预检能力（按 spec）

- [x] 6.1 更新 `/api/preflight/cli` 的响应：包含 `git` 可用性诊断（并保持对现有字段的向后兼容或同步更新前端）
- [x] 6.2 增加 `git` 不可用时的回归测试（HTTP 与 MCP tool 至少覆盖一个入口）
- [x] 6.3 如实现包含 GitHub CLI 认证检测，补齐 `gh auth status` 相关测试（可用性/未登录/已登录）

Verification:
- `cargo test -p server cli_dependency_preflight`
- `pnpm -C frontend lint`

## 7. 全量验证与闭环

- [x] 7.1 运行 `just qa`
- [x] 7.2 运行 `just openspec-check`
- [x] 7.3 确认所有新增/修改 spec 场景均有对应测试或验收步骤
