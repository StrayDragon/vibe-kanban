## Context

当前 `frontend/src/main.tsx` 在生产环境会无条件静态 import 并渲染：

- `click-to-react-component`（调试定位工具，典型 dev-only）
- `vibe-kanban-web-companion`（开发辅助/伴随组件）

静态 import 会把依赖及其传递依赖绑定进生产构建产物，抬高首屏 JS 体积与 parse/eval 成本；同时这些组件在运行期也会增加常驻的 CPU/内存占用。

## Goals / Non-Goals

**Goals:**
- 生产构建不再通过入口静态依赖引入 dev-only 包。
- 生产运行不渲染 dev-only 组件。
- 开发环境保留同等体验（仍可用上述工具）。
- 增加轻量测试，避免入口回归到静态 import。

**Non-Goals:**
- 不调整依赖安装策略（例如把包从 dependencies 移到 devDependencies），本变更只保证生产 bundle 不包含它们。
- 不对 router/页面级 code-splitting 做额外重构（只处理入口与 dev-only 工具的引入方式）。

## Decisions

1) **入口改为 DEV 下按需加载**
- 在 `frontend/src/main.tsx` 中移除对 dev-only 包的静态 import。
- 新增 `frontend/src/dev/DevOnlyRoot.tsx`（仅开发使用），集中 import 并渲染 `ClickToComponent` 与 `VibeKanbanWebCompanion`。
- 在 `main.tsx` 里仅当 `import.meta.env.DEV` 为 true 时，通过 `React.lazy(() => import('./dev/DevOnlyRoot'))` 动态加载，并用 `<React.Suspense fallback={null}>` 包裹。

选择该方案的原因：
- `import.meta.env.DEV` 在生产构建中为编译期常量（false），可被 tree-shaking 直接裁剪整个 dev-only 分支，避免生成额外 chunk 或被动包含依赖。
- 保持代码结构清晰：dev-only 依赖集中在 `frontend/src/dev/*`，入口文件只保留最小胶水。

2) **以测试作为“入口静态依赖”护栏**
- 新增 Vitest 测试读取 `frontend/src/main.tsx` 源码，断言不出现对 `click-to-react-component` / `vibe-kanban-web-companion` 的静态 import。
- 同时断言存在 dev-only 动态 import（确保功能未被误删且仍是按需加载）。

## Risks / Trade-offs

- [Risk] bundler 未正确裁剪 DEV 分支导致生产仍打包 dev-only 代码 → Mitigation：入口静态 import 测试 + `pnpm -C frontend run build` 后在产物中快速检索关键字符串作为人工验收（由 `just qa` 覆盖构建）。
- [Trade-off] 开发环境首次加载 dev-only 工具多一次动态 import → 可接受（dev-only），并且不会影响生产首屏。

