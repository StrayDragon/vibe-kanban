## Context

Vibe Kanban 的核心价值是本地任务编排、attempt/workspace 生命周期、日志与变更可观测性。与远程仓库（GitHub 等）的 PR 交互属于附加集成能力，涉及：
- 网络依赖与速率限制
- 鉴权与 token 生命周期
- 第三方 API 变化导致的维护与回归

在“最小 + 强大核心”的方向下，我们希望将这类能力从 core server 的 HTTP API surface 中移除，避免未来协议/依赖升级时需要持续跟进 PR 交互细节。

## Goals / Non-Goals

**Goals:**
- 移除 GitHub PR 交互相关 endpoints 与前端入口，减少副作用能力面与维护成本。
- 让 task-attempt 核心能力（创建/状态/日志/变更/git/worktree/执行）保持稳定。
- UI 保留“复制信息”帮助用户完成 PR 工作流，但不由 VK 发起远程 API 调用。

**Non-Goals:**
- 不删除在线翻译、诊断检查、文件系统浏览、repo 注册/初始化/分支枚举、attempt 执行型 API、scratch、里程碑。
- 不改动本地 git 操作接口（push/rebase/merge 等仍按现状）。
- 不新增任何“快捷打开/远程触发本机副作用”接口。

## Decisions

1. **彻底移除 PR endpoints 的路由挂载**
   - 选择：从 task-attempts router 中移除 PR 子路由；对应 handler/DTO 与前端调用一并删除。
   - 原因：真正缩小 attack surface；不再需要在 access control/DTO redaction 上为 PR 能力兜底。
   - 备选：保留 endpoint 但返回 410。若迁移期需要更友好错误，可在短窗口内使用 410；最终仍删除路由。

2. **前端将 PR 工作流降级为“复制信息 + 教程”**
   - 选择：UI 只展示 attempt 的分支名、可能的 compare URL 模板、建议命令（例如 `git push`），并一键复制。
   - 原因：用户仍能快速完成 PR，但系统不承担远程交互与鉴权维护。

3. **保持配置字段最小化**
   - 选择：不引入新的 github 配置写入；若现有 `github.pat` 等字段仍被其他能力需要则保留，否则在后续变更中再评估移除。

## Risks / Trade-offs

- [兼容性] 旧前端/脚本调用 PR endpoints 将失败。
  - 缓解：前端同步移除入口；必要时短期提供 410 + 清晰错误信息（“该能力已移除，改用复制命令”）。
- [体验] 一键 PR 能力消失。
  - 缓解：copy-friendly 的分支名/compare URL/命令模板；文档提示如何手工创建/绑定 PR。

## Migration Plan

1. 后端删除 PR routes/handlers/DTO，确保 `cargo test` 通过。
2. 前端删除 PR UI/hook，确保 `pnpm -C frontend run check` 通过。
3. 文档/提示更新：将 PR 相关入口替换为复制信息与教程。
4. 可选：短窗口提供 410（Gone）响应帮助排障；后续版本完全删除路由。

## Open Questions

- 是否需要保留一个“PR 信息摘要”只读 endpoint（不访问远端，只拼装 URL 模板）？
  - 当前倾向：不需要，直接由前端基于 branch/repo 信息生成即可。

