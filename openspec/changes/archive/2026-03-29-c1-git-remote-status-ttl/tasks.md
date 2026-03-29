## 1. Spec & 设计落盘

- [ ] 1.1 完成 `proposal.md`（明确为何需要 TTL gate + 最小 fetch）
- [ ] 1.2 完成 `specs/git-remote-status-ttl/spec.md`（新增规范：TTL gate、并发不阻塞、失败降级）
- [ ] 1.3 完成 `design.md`（决策：TTL gate 位置、refspec 收敛、失败冷却）

## 2. 后端实现

- [ ] 2.1 在 `crates/repos/src/git/mod.rs` 为 `get_remote_branch_status` 增加 TTL gate（env：`VK_GIT_REMOTE_STATUS_FETCH_TTL_SECS`）
- [ ] 2.2 将远端 fetch refspec 收敛为单分支（不再 fetch `refs/heads/*`）
- [ ] 2.3 确保 fetch 失败时 branch-status 仍可返回（远端字段允许为 `null`），并增加失败冷却窗口

## 3. 测试覆盖

- [ ] 3.1 在 `crates/repos` 增加本地 bare remote 的回归测试：TTL 内不重复 fetch、TTL 外可再次刷新并反映远端变化
- [ ] 3.2 覆盖带斜杠分支名与无 upstream 的降级路径（不 panic）

## 4. 验收与归档

- [ ] 4.1 运行：`cargo test -p repos`
- [ ] 4.2 运行：`cargo test -p server`
- [ ] 4.3 运行：`just qa`
- [ ] 4.4 运行：`just openspec-check`
- [ ] 4.5 通过后 archive/sync 该 change
- [ ] 4.6 创建最终 commit：`refactor: git-remote-status-ttl`

