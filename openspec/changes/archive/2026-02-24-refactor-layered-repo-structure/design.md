# Design: Layered repo structure refactor

## Goals
- 将“入口/路由/DTO/handlers”与“业务服务逻辑”与“数据访问/模型”分离
- 降低单文件规模，强化模块边界与可检索性
- 保持对外 API 行为稳定（路径、方法、JSON 结构），将破坏性集中在内部目录与导入路径

## Non-Goals
- 不引入新业务功能
- 不改变数据库 schema 与既有迁移
- 不更换前端框架/路由库/状态管理方案

## Backend layering rules

### crates/server
**职责**：HTTP/API 边界（参数解析、校验、调用 services、响应映射）

**目录约定**
- `crates/server/src/http/`
  - 组装 axum Router（含 `/api` nest 与静态资源路由）
  - 放中间件与 extractor（如 workspace loader）
- `crates/server/src/api/<resource>/`
  - `router.rs`：只做路由 wiring（axum Router）
  - `handlers.rs`：只做 HTTP handlers（不做复杂业务逻辑）
  - `dto.rs`：request/response/query 类型（含 ts-rs TS）
  - `ws.rs`：ws/sse（如需要）

**依赖边界**
- handlers 可以调用 `services::*`
- handlers 不应直接执行复杂 SQL/事务编排；此类逻辑应在 `services` 内封装
- server 不应依赖 frontend

### crates/services
**职责**：业务服务层，承载跨模型/跨 repo 的 orchestration。

**目录约定**
- 超过 ~800 行的服务必须目录化拆分（例如 `container/`、`git/`）
- 对外暴露一个清晰外观（Facade）：`pub struct XService` + `mod.rs` re-export

### crates/db
**职责**：模型 + 基础查询/校验 + 领域错误类型。

**目录约定**
- 超大模型（如 task/task_group）拆为目录：`types.rs`、`queries.rs`、`errors.rs`、`mod.rs`
- 对外尽量保持 `db::models::task::*` 等导出路径稳定

### crates/executors
**职责**：对不同 agent 的适配与日志归一化等。

**目录约定**
- executor 按目录组织，`mod.rs` 保持对外类型名稳定（例如 `ClaudeCode`）

## Frontend layering rules

### frontend/src/api
**职责**：所有 API 调用入口（fetch/client + 各领域 api），禁止在 pages/components 里散落拼 URL。

### frontend/src/app
**职责**：应用组装层（providers/router），不放业务逻辑。

### frontend/src/pages
**职责**：路由页面；超大页面必须目录化拆分：`index.tsx` + `components/*` + `state.ts`/`utils.ts`。

### frontend/src/components
**职责**：可复用组件；`components/ui` 只放 UI primitives。

### frontend/src/hooks
**职责**：hooks 按领域分组子目录；对外通过 `hooks/index.ts` 聚合导出，减少导入变更面。

## Compatibility strategy
- `/api/*` 路由路径与响应 JSON 形状默认保持不变
- `shared/types.ts` 由 generator 更新；如果出现结构性变化，必须在 proposal 追加说明与迁移策略

