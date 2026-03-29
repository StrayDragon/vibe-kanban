## 1. One-shot optimistic resync 调度

- [x] 1.1 提取“下一次 eligible resync 时间”计算为纯函数，并在 `useAllTasks` 中用 one-shot timer 替换 250ms tick（验证：`pnpm -C frontend test`）
- [x] 1.2 在 `useProjectTasks` 中同样替换为 one-shot timer（保持 connectEnabled 门控语义一致）（验证：`pnpm -C frontend test`）

## 2. 测试

- [x] 2.1 新增 Vitest 单测覆盖调度计算的边界：attempts 上限、min gap、resyncAfter 门控（验证：`pnpm -C frontend test`）

## 3. 验收 / 归档 / 提交

- [x] 3.1 运行并修复直到通过：`just qa`、`just openspec-check`
- [x] 3.2 归档该 change（`openspec archive -y c3-frontend-optimistic-resync-timer`）并创建最终 commit：`refactor: frontend-optimistic-resync-timer`
