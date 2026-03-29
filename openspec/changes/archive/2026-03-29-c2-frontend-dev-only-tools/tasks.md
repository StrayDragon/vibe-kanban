## Tasks

- [x] 1. 移除入口对 dev-only 依赖的静态 import
  - [x] 更新 `frontend/src/main.tsx`：删除 `click-to-react-component` / `vibe-kanban-web-companion` 的静态 import 与渲染
  - [x] 新增 `frontend/src/dev/DevOnlyRoot.tsx`：集中渲染 dev-only 工具
  - [x] 在 `main.tsx` 中仅在 `import.meta.env.DEV` 下通过 `React.lazy` 动态引入 `./dev/DevOnlyRoot`

- [x] 2. 增加测试护栏（防回归）
  - [x] Vitest：断言 `frontend/src/main.tsx` 不包含对上述包的静态 import
  - [x] Vitest：断言 `main.tsx` 存在对 `./dev/DevOnlyRoot` 的动态 import（确保仍为按需加载）

- [x] 3. 验收与验证
  - [x] 运行 `pnpm -C frontend run test`
  - [x] 运行 `pnpm -C frontend run build`
  - [x] 运行 `just qa`
  - [x] 运行 `just openspec-check`

- [x] 4. 归档与提交
  - [x] `openspec archive -y c2-frontend-dev-only-tools`
  - [x] 创建 commit：`refactor: frontend-dev-only-tools`
