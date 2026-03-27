## Why

当前 task attempt 的 GitHub PR 交互（创建/绑定 PR、拉取评论等）属于“远程网络副作用 + 凭据依赖”的能力面：需要维护 GitHub API/权限/鉴权细节，容易在协议与依赖升级时产生回归，同时也放大了泄露面与故障面（网络错误、token 过期/权限不足导致 UI/流程不稳定）。

我们希望保持 **最小但强大** 的本地核心：保留本地 git/worktree/attempt 机制与依赖，但移除对远程仓库 PR 的直接交互，让 PR 流程回归为“复制信息 + 用户自行操作”，从而降低长期维护成本与风险。

## What Changes

- **BREAKING**: 移除 task-attempt 的 GitHub PR 相关 HTTP API（创建/绑定 PR、获取 PR 评论等）。
- **BREAKING**: 前端移除/隐藏 attempt 页面与相关对话框中的 “GitHub PR” 交互入口，改为提供更安全的“复制分支名/复制 compare URL 模板/复制建议命令”等只读辅助信息（不触发网络副作用）。
- 清理后端 GitHub PR 交互相关实现与依赖链（不影响本地核心 attempt、日志、变更、git 操作等能力）。

Goals:
- 将远程 PR 交互从核心服务中移除，避免网络/鉴权耦合与维护负担。
- 保留本地核心机制（attempt/git/worktree/日志/变更等）不回归。
- UI 仍能给用户提供完成 PR 工作流所需的最小信息（复制即可）。

Non-goals:
- 不删除在线翻译、诊断检查、文件系统浏览、repo 注册/初始化/分支枚举、attempt 执行型 API、scratch、里程碑等能力面。
- 不改变本地 git 操作能力（push/rebase/merge 等仍保留）。
- 不引入新的“远程触发本机副作用”入口。

Risks:
- [BREAKING] 依赖 PR 接口的旧前端/脚本会 404/失败。
  - Mitigation: 前端同步移除入口；必要时将旧 endpoint 短期改为明确的 410 + 可操作错误信息（而非静默 404）。
- PR 流程便利性下降。
  - Mitigation: UI 提供 copy-friendly 的分支名/compare URL/命令模板，减少手工成本。

Verification:
- `cargo test -p server task_attempts`
- `pnpm -C frontend run check`
- 手动验证：attempt 页面不再出现 PR 按钮/对话框；调用旧 PR endpoint 返回 404/410。

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `task-attempts`: 明确 task-attempt 核心 API surface 的范围，并移除远程 GitHub PR 交互类 endpoints（作为非核心集成能力不再提供）。

## Impact

- Backend:
  - `crates/server/src/routes/task_attempts/pr.rs`（以及 task_attempts router wiring）
  - 可能涉及 `execution::github::GitHubService` 的引用清理
  - TS types 生成（若 PR 相关 DTO 当前对外导出）
- Frontend:
  - attempt 页面 PR 相关 UI/hook/api
  - 文案与帮助指引（改为复制信息而非发起网络操作）

