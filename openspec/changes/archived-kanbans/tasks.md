## 1. 数据库与模型

- [x] 1.1 添加 SeaORM migration：创建 `archived_kanbans` 表 + 为 `tasks` 增加可空外键 `tasks.archived_kanban_id` + 索引（验证：`cargo test -p db-migration` 或 `cargo check -p db-migration`）
- [x] 1.2 添加 ArchivedKanban 的 db entity/model（`crates/db/src/entities/archived_kanban.rs`、`crates/db/src/models/archived_kanban.rs`），并在 `crates/db/src/entities/mod.rs` + `crates/db/src/models/mod.rs` 导出（验证：`cargo check -p db`）
- [x] 1.3 扩展 Task 的 db entity/model：增加 `archived_kanban_id`，并确保 ts-rs 类型生成覆盖 `Task` / `TaskWithAttemptStatus`（验证：`cargo check -p db`）

## 2. 后端 HTTP API（Axum）

- [x] 2.1 添加归档相关 HTTP 路由：
  - `GET /api/projects/:project_id/archived-kanbans`（列表）
  - `GET /api/archived-kanbans/:id`（详情/元数据）
  - `POST /api/projects/:project_id/archived-kanbans`（按 status 归档 + 可选标题）
  - `POST /api/archived-kanbans/:id/restore`（restore-all / restore-by-status）
  - `DELETE /api/archived-kanbans/:id`（同步硬删除归档 + tasks）
  （验证：为 `crates/server/src/routes/*` 增加/扩展路由测试，并运行 `cargo test -p server`）
- [x] 2.2 增加任务列表过滤：
  - 扩展 `/api/tasks` 与 `/api/tasks/stream/ws`：默认排除归档任务
  - 支持显式 `include_archived=true` 与 `archived_kanban_id=<uuid>` 过滤
  （验证：`cargo test -p server` 增加测试，断言默认不返回归档任务）
- [x] 2.3 服务端护栏：归档任务不可写/不可执行
  - 对 `archived_kanban_id != NULL` 的任务拒绝 update/delete
  - 对归档任务拒绝 attempt 创建（`/api/tasks/create-and-start`、attempt start handlers）
  （验证：`cargo test -p server` 覆盖 update/delete/create-and-start 的拒绝逻辑）
- [x] 2.4 归档/删除归档的运行中进程预检查
  - 归档 MUST 在匹配集合存在运行中执行进程时拒绝
  - 删除归档 MUST 在归档内存在运行中执行进程时拒绝
  （验证：`cargo test -p server` 增加冲突测试）

## 3. MCP 工具

- [x] 3.1 添加 archived-kanbans MCP tools（含 schema）：
  - `list_archived_kanbans(project_id)`
  - `archive_project_kanban(project_id, statuses, title?)`
  - `restore_archived_kanban(archive_id, restore_all?, statuses?)`
  - `delete_archived_kanban(archive_id)`
  要求 `structuredContent` + `outputSchema`，破坏性 tools 标注 `destructiveHint=true`（验证：在 `crates/server/src/mcp/task_server.rs`（或相邻 test module）增加 MCP tool 测试）
- [x] 3.2 MCP 路径同样禁止对归档任务执行（验证：增加 MCP 级别测试，断言对归档任务 start attempt 失败且返回结构化错误）

## 4. 前端 UI

- [x] 4.1 添加归档前端 API client（例如 `frontend/src/api/archived-kanbans.ts`），并按需在 `frontend/src/api/index.ts` 导出（验证：`pnpm -C frontend run check`）
- [x] 4.2 添加 Project 级别路由与页面：
  - `/projects/:projectId/archives`（列表）
  - `/projects/:projectId/archives/:archiveId`（只读归档看板详情）
  （验证：在 dev server 中手工冒烟检查）
- [x] 4.3 在 Project 看板视图增加“归档”动作：
  - 对话框：可选标题 + 可选 statuses（默认 `done`/`cancelled`）
  - 确认文案明确说明“不可变 + 不可执行”
  （验证：手工冒烟检查 + 无 TS 错误）
- [x] 4.4 归档详情必须强只读：
  - 不允许拖拽、编辑、创建任务、start attempt
  - 提供“还原”与“删除归档”动作
  （验证：手工冒烟检查）
- [x] 4.5 还原对话框：
  - restore-all 或 restore-by-status
  - 还原后任务在活跃看板重新出现，且不改变 status
  （验证：手工冒烟检查）
- [x] 4.6 删除归档流程：
  - 高摩擦确认（建议输入确认文本）
  - 正确处理“存在运行中进程”的冲突错误
  （验证：手工冒烟检查）
- [x] 4.7 All Tasks 视图更新：
  - 默认排除归档任务
  - 增加开关以包含归档任务（验证：手工冒烟检查 + `pnpm -C frontend run check`）

## 5. 共享类型（ts-rs）

- [x] 5.1 在 `crates/server/src/bin/generate_types.rs` 接入新类型，并通过 `pnpm run generate-types` 重新生成 `shared/types.ts`（验证：`pnpm -C frontend run check`）

## 6. 测试与验证

- [x] 6.1 后端检查/测试（验证：`pnpm run backend:check` 与 `cargo test --workspace`）
- [x] 6.2 前端检查/格式（验证：`pnpm -C frontend run check` 与 `pnpm -C frontend run lint`）

## 7. OpenSpec 工件（全中文重制）

- [x] 7.1 补回并重写 `openspec/changes/archived-kanbans/proposal.md`，补充“不确定性与未知情况处理”说明（验证：`openspec instructions apply --change "archived-kanbans" --json`）
- [x] 7.2 全中文重制 `design.md` / `spec.md` / `tasks.md`，并同步关键决策（任务组原子化、安全阀、默认过滤、WS 收敛等）（验证：`openspec instructions apply --change "archived-kanbans" --json`）
