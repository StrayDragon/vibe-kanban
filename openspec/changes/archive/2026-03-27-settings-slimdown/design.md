## Context

Settings 当前已倾向只读/引导，但实现仍分散在多页，多处重复实现 copy/reload 逻辑，并存在重复 WS 订阅与敏感路径直出的问题。我们希望通过页面收敛与逻辑复用，让 Settings 成为稳定的“最小入口”，降低维护与回归。

## Goals / Non-Goals

**Goals:**
- 单页 Settings（或单路由 + 锚点）承载 Config/Projects/MCP 三块内容。
- 全站唯一 reload 入口，统一 toast/错误处理。
- Projects Settings 复用 ProjectContext，不重复 WS。
- 默认不直出 secret.env 绝对路径，但保留复制能力。
- copyToClipboard 逻辑统一抽取，减少重复与文案漂移。

**Non-Goals:**
- 不改后端 API contract（除非为支持安全展示需要调整字段）。
- 不新增可远程触发本机副作用（open file/open editor 等）。

## Decisions

1. **Settings 路由收敛**
   - 选择：保留单路由 `/settings`，内部用锚点或 tabs 分区；移除 `/settings/agents` 子页。
   - 备选：保留多页但统一复用组件。考虑到“最小核心”，优先单页。

2. **唯一 reload 入口**
   - 选择：仅在 Config 区块提供 reload 按钮，其余页面/区块不再重复提供。
   - 理由：避免多个入口在错误处理/加载态/权限提示上出现漂移。

3. **Projects 数据源复用**
   - 选择：ProjectProvider 统一建立 WS 与 patch apply；Settings Projects 仅消费 context state。
   - 理由：减少长连接数量与断线重连复杂度。

4. **路径展示策略**
   - 选择：默认仅展示文件名/相对提示；提供“复制完整路径”按钮。\n     secret.env 路径不默认直出（避免旁观泄露与截图泄露）。

5. **复制能力统一**
   - 选择：新增 `useCopyToClipboard()`（或组件），集中 toast 文案与错误处理，替换 Settings 多处实现。
6. **不做“高级/调试区”折叠**
   - 选择：Settings 单页直接把 Config/Projects/MCP 三块信息清晰展示；只提供一个“主动 reload 配置”的按钮入口，不额外引入复杂的折叠层级与多处重复入口。

## Risks / Trade-offs

- [深链] 老的 Settings 子路由可能失效。
  - 缓解：保留重定向或在单页中识别 query/hash 并跳转到对应区块。
- [排障] 路径不直出可能降低某些排障效率。
  - 缓解：复制按钮仍在，且可在高级/折叠区显示。

## Migration Plan

1. 新增 copy hook/组件并替换现有 Settings 页面内的重复实现。
2. 合并/删除 Settings 子页与路由收敛；必要时增加重定向。
3. Projects Settings 改为消费 ProjectContext（去掉额外 WS）。
4. 调整路径展示策略与文案，跑 `pnpm -C frontend run check`。
