## 1. DB + Models（数据库与模型）

- [ ] 1.1 新增 SeaORM migration，为 `sessions` 表添加可空列 `name`
- [ ] 1.2 更新 `crates/db/src/entities/session.rs` 与 `crates/db/src/models/session.rs`，加入 `name`
- [ ] 1.3 在关键的 session 创建调用点，在上下文可得时传入自动生成的 `name`

## 2. API

- [ ] 2.1 扩展 `/api/sessions` 返回包含 `name`
- [ ] 2.2 增加 `PATCH /api/sessions/:session_id` 重命名接口并做校验（trim、空字符串 → null、max length）
- [ ] 2.3 在前端 API client 增加 `sessionsApi.rename(sessionId, { name })`

## 3. Frontend（Processes Dialog）

- [ ] 3.1 获取当前 attempt/workspace 的 sessions，并展示 selector（显示 `name` 或 fallback label）
- [ ] 3.2 为选中的 session 增加重命名 UI（dialog 或 inline edit）
- [ ] 3.3 （可选）按选中的 session id 过滤 execution process 列表

## 4. Types + Verification

- [ ] 4.1 运行 `pnpm run generate-types`，并确保 `shared/types.ts` 的更新被提交
- [ ] 4.2 运行 `pnpm run check` 与 `pnpm run lint`
- [ ] 4.3 运行 `cargo test --workspace`
