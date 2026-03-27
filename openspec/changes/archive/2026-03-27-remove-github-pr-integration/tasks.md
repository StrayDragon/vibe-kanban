## 1. 后端：移除 GitHub PR routes/handlers

- [x] 1.1 从 task-attempts router 中移除 PR 相关路由挂载（确保旧路径返回 404/必要时 410）
- [x] 1.2 删除/内联 `crates/server/src/routes/task_attempts/pr.rs` 及相关 DTO/Service wiring（确保无死代码/无未使用依赖）
- [x] 1.3 清理 `execution::github::GitHubService` 等仅为 PR 功能存在的依赖引用（若仍被其他能力需要则保留）

Verification:
- `cargo test -p server task_attempts`
- `cargo test --workspace`

## 2. 前端：移除 PR 入口，改为复制信息

- [x] 2.1 移除 attempt 页面/对话框中 PR 创建/绑定/评论相关 UI 与调用链（API hooks、组件、文案）
- [x] 2.2 以只读方式提供最小 PR 工作流辅助信息（复制分支名、复制 compare URL 模板、复制建议命令），不触发远程请求
- [x] 2.3 运行并修复类型与编译检查（如 TS types 变更）

Verification:
- `pnpm -C frontend run check`

## 3. 回归测试与文档

- [x] 3.1 增加回归测试：调用旧 PR endpoint 返回 404/410（且不会触发网络副作用）
- [x] 3.2 更新 Settings/帮助文案：PR 相关指引改为“复制信息 + 手工操作”模式

Verification:
- `cargo test -p server`
- 手动验证：attempt 页面无 PR 入口；旧 PR endpoint 404/410
