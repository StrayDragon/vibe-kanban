## 1. Backend：暴露 `number` 与 `short_id`

- [ ] 1.1 扩展 `db::models::task::Task`，增加 `number` 与 `short_id` 字段
- [ ] 1.2 `number` 从 DB primary key 取值，`short_id` 从 UUID prefix 派生
- [ ] 1.3 运行 `pnpm run generate-types`，并修复前端编译错误（如有）

## 2. Frontend：Search + Display

- [ ] 2.1 更新 `frontend/src/pages/ProjectTasks.tsx` 的 search matcher：支持 `#<number>`/`<number>` 以及 `short_id`/UUID prefix 匹配
- [ ] 2.2 在 kanban task cards（以及可选的 task detail header）展示 `#<number>`
- [ ] 2.3 （如 `frontend/src/pages/` 下存在合适的测试框架）为 search matcher 添加一个小的 unit test

## 3. Verification

- [ ] 3.1 运行 `pnpm run check` 与 `pnpm run lint`
- [ ] 3.2 运行 `cargo test --workspace`
