## 0. Scope & Guardrails
- [x] 0.1 保持 `/api/*` 路径与 JSON 形状兼容（除非明确记录为 breaking）
- [x] 0.2 `shared/types.ts` 不手改；通过 `pnpm run generate-types` 生成
- [x] 0.3 每个迁移阶段都跑一次：`cargo check --workspace` + `pnpm -C frontend check`

## 1. OpenSpec housekeeping
- [x] 1.1 补齐各 capability delta spec（至少：task-attempts、execution-logs、config-management、workflow-orchestration）
- [x] 1.2 `openspec validate refactor-layered-repo-structure --strict`

## 2. Frontend refactor
- [x] 2.1 将 `frontend/src/lib/api.ts` 拆分为 `frontend/src/api/*`（client + domain APIs + index re-export）
- [x] 2.2 引入 `frontend/src/app/*`（providers/router/App）
- [x] 2.3 大页面目录化：`TasksOverview`、`TaskGroupWorkflow`、`pages/settings/*`
- [x] 2.4 新增 alias：`@api/*`、`@app/*`（tsconfig + vite）

## 3. Backend refactor: server layering
- [x] 3.1 新增 `crates/server/src/http/*` 与 `crates/server/src/api/*` 骨架
- [x] 3.2 将 `routes/mod.rs` 路由组装迁移至 `http/mod.rs`
- [x] 3.3 迁移与拆分 `task_attempts` 等超大模块为目录结构（router/handlers/dto/ws）
- [x] 3.4 统一 `ApiError` 入口位置（保持行为不变）

## 4. Backend refactor: services/db/executors hotspots
- [x] 4.1 `crates/services/src/services/container.rs` → `container/` 目录化拆分
- [x] 4.2 `crates/services/src/services/git.rs` → `git/` 目录化拆分
- [x] 4.3 `crates/db/src/models/task.rs` 与 `task_group.rs` 目录化拆分
- [x] 4.4 executors：至少目录化梳理 `claude` 与 `codex normalize` 的结构与 re-export

## 5. Types + Validation
- [x] 5.1 修正 generator 注释/导入路径并运行 `pnpm run generate-types`
- [x] 5.2 跑全量检查：Rust fmt/clippy/test + frontend lint/test/check + generate-types:check

## 6. Docs
- [x] 6.1 更新 `ARCH.md` 目录结构与查找指南
- [x] 6.2 如 README 引用旧路径，更新为新入口
