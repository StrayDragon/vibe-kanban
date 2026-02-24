# Change: Layered repo structure refactor (frontend + backend)

## Why
当前仓库前后端都存在“单文件过大、模块边界模糊、跨层引用随意”的维护成本问题，例如：
- `crates/server/src/routes/task_attempts/handlers.rs`、`crates/services/src/services/container/mod.rs`、`crates/services/src/services/git/mod.rs` 体积过大且职责混杂
- `frontend/src/lib/api.ts` 集中承载所有 API 调用与错误处理，导致改动面大、复用与测试困难
- 部分概念在不同层的命名不一致（例如 attempt/workspace），需要通过更清晰的层次与封装统一语义

## What Changes
- **Backend (Rust)**
  - 在 `crates/server` 引入明确的分层：`http/`（路由组装与中间件）与 `api/`（资源模块：router/handlers/dto/ws）
  - 将超大模块拆分为目录化子模块（server routes、services container/git、db 的 task/task_group、executors 的 claude/codex 等）
  - 统一错误处理入口与映射位置（保持现有 HTTP 路径与响应形状不变）
- **Frontend (React/TS)**
  - 将 `frontend/src/lib/api.ts` 拆分为 `frontend/src/api/*`（client + 各领域 API），并统一 re-export
  - 引入 `frontend/src/app/*`（providers/router/App 组装层），将页面与组装逻辑解耦
  - 将超大页面目录化（`TasksOverview`、`TaskGroupWorkflow`、`settings/*` 等）
  - 增加 TS/Vite alias：`@app/*`、`@api/*`
- **Types**
  - 继续通过 `crates/server/src/bin/generate_types.rs` 生成 `shared/types.ts`，不手改生成物

## Impact
- 影响范围：全仓库（Rust workspace + frontend）
- 兼容性：默认保持 `/api/*` 路径与 JSON 形状兼容；允许大量内部模块路径与 TS 导入路径变更
- 风险：一次性大迁移可能导致合并冲突与漏改导入；通过分阶段编译/类型检查与全量测试缓解
