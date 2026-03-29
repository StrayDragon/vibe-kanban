## Why

当前前端入口 `frontend/src/main.tsx` 会在生产环境无条件引入并渲染 dev-only 的调试/伴随组件（例如 `click-to-react-component`、`vibe-kanban-web-companion`）。这会抬高首屏 JS 体积与初始化开销，并增加运行期 CPU/内存占用。

我们需要把这些“仅开发期需要”的能力从生产常驻链路移除，形成可持续的性能护栏，避免回归。

## What Changes

- 移除 `frontend/src/main.tsx` 对 dev-only 组件的静态 import，并在生产环境不渲染这些组件。
- 在开发环境（`import.meta.env.DEV`）下按需加载并渲染 dev-only 组件，保持开发体验。
- 增加前端测试用例，防止入口文件重新出现对 dev-only 包的静态依赖（避免生产 bundle 被动包含）。

## Capabilities

### New Capabilities

- （无）

### Modified Capabilities

- `frontend-performance-guardrails`: 增加“dev-only 工具不进入生产常驻链路”的要求（生产构建不应通过入口静态依赖引入这些包，且生产运行不渲染）。

## Impact

- 影响前端入口与少量组件代码：`frontend/src/main.tsx`，新增一个 dev-only 的渲染入口模块（例如 `frontend/src/dev/devOnlyRoot.tsx`）。
- 不影响后端 API；仅改变前端生产包的依赖与运行期行为。
- 验证方式：`pnpm -C frontend run test`、`pnpm -C frontend run build`（在 `just qa` 中覆盖）。

