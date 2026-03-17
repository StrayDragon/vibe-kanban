## 1. Backend: Expose `number` and `short_id`

- [ ] 1.1 Extend `db::models::task::Task` to include `number` and `short_id`
      fields
- [ ] 1.2 Populate `number` from the DB primary key and `short_id` from the UUID
      prefix
- [ ] 1.3 Run `pnpm run generate-types` and update any frontend compile errors

## 2. Frontend: Search + Display

- [ ] 2.1 Update `frontend/src/pages/ProjectTasks.tsx` search matcher to include
      `#<number>`/`<number>` and `short_id`/UUID prefix matching
- [ ] 2.2 Display `#<number>` in kanban task cards and (optionally) task detail
      header
- [ ] 2.3 Add a small unit test for the search matcher (if a suitable test
      harness exists in `frontend/src/pages/`)

## 3. Verification

- [ ] 3.1 Run `pnpm run check` and `pnpm run lint`
- [ ] 3.2 Run `cargo test --workspace`

