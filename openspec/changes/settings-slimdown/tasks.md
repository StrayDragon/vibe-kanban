## 1. Settings 页面收敛（单页三分区）

- [ ] 1.1 将 Settings 收敛为单路由/单页（Config / Projects / MCP 三个区块），更新 `AppRouter` 与 Settings layout
- [ ] 1.2 移除 `/settings/agents` 页面并将必要内容并入 Config 区块（必要时保留重定向到新页面锚点）

Verification:
- `pnpm -C frontend run check`

## 2. 唯一 reload 入口

- [ ] 2.1 全站只保留一个 config reload 按钮（统一 loading/toast/错误提示）
- [ ] 2.2 删除 Settings 其它区块/页面中重复的 reload mutation 逻辑

Verification:
- 手动验证：Settings 中只有一个 reload 按钮；失败提示一致

## 3. Projects 设置去重 WS（复用 ProjectContext）

- [ ] 3.1 `ProjectSettings` 改为消费 `ProjectContext`，移除重复 `useProjects()` 调用
- [ ] 3.2 确认 `useJsonPatchWsStream` 不会因为 Settings 页面渲染导致重复连接

Verification:
- 手动验证：打开 Settings 不新增额外 WS（开发者工具网络/日志观察）

## 4. 路径展示与复制能力统一

- [ ] 4.1 默认不直出 `secret.env` 绝对路径（可折叠/仅文件名），但保留“一键复制路径”
- [ ] 4.2 抽取统一 `copyToClipboard` hook/组件，替换 Settings 内重复实现（General/Projects/MCP）
- [ ] 4.3 Projects snippet generator 更静态化（减少状态/逻辑），保持“一键复制即可用”

Verification:
- `pnpm -C frontend run check`

