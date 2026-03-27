## Why

当前 Settings 体验与实现仍偏“多页 + 多入口 + 多状态源”：
- 多个 Settings 页面分别维护 reload/复制/状态展示逻辑，重复代码与行为漂移风险高。
- Projects Settings 可能重复建立 projects WS（全局 context 已订阅时仍再次订阅），增加性能与断线重连复杂度。
- UI 直出绝对路径（尤其 `secret.env`）会扩大信息暴露面，也增加界面噪音。

我们希望把 Settings 收敛为“只读 + 教程 + 一键复制”的最小结构，减少维护与回归点，并降低潜在泄露面。

## What Changes

- Settings 收敛为单页 3 个区块：\n  1) Config（状态/错误/dirty 提示 + 单一 reload 入口）\n  2) Projects（只读列表 + snippet 复制 + 指引）\n  3) MCP（教程 + snippet 复制）
- 移除 `/settings/agents`（并入 Config/General 区块），全站只保留一个 reload 入口，避免多处行为不一致。
- Projects Settings 复用全局 ProjectContext 数据，不再额外 `useProjects()` 建立第二条 WS。
- 默认不直出 `secret.env` 绝对路径：可折叠/仅文件名/可复制但不展示（按安全与体验取舍）。
- 抽取统一 `copyToClipboard` hook/组件，删除 Settings 内多处重复实现；Projects snippet generator 更静态化。

Goals:
- Settings 代码量显著下降，行为更一致。
- 减少长连接与重复订阅，提高前端稳定性与性能。
- 降低敏感路径展示面（尤其 secret.env）。

Non-goals:
- 不改变后端配置读写策略（仍是 file-first YAML，UI 不写配置）。
- 不删除在线翻译、诊断检查等其它页面功能（本变更只聚焦 Settings 收敛）。

Risks:
- Settings 路由结构变化可能影响书签/深链。
  - Mitigation: 可保留旧路由做重定向到新单页锚点（可选）。
- 折叠/隐藏路径可能影响部分排障习惯。
  - Mitigation: 仍支持“一键复制路径”，仅不默认展示。

Verification:
- `pnpm -C frontend run check`
- 手动验证：Settings 单页可用；reload 入口唯一；Projects 不重复 WS；复制功能正常。

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `project-settings-summary`: Settings > Projects 体验从“可编辑/多入口”收敛为“只读元信息 + file-first 指引 + 复制 snippet”，并统一指向 `projects.yaml` / `projects.d/*.yaml`。

## Impact

- Frontend:
  - `frontend/src/app/AppRouter.tsx`（Settings 路由结构）
  - `frontend/src/pages/settings/*`（页面合并/删除）
  - `frontend/src/contexts/ProjectContext.tsx` / `useProjects`（避免重复 WS）
  - 公共复制 hook/组件新增

