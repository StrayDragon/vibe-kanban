## 1. Workspace cleanup configurability (local deployment)

- [x] 1.1 Add `VK_WORKSPACE_EXPIRED_TTL_SECS` and `DISABLE_WORKSPACE_EXPIRED_CLEANUP` to control TTL-based cleanup. (verify: `cargo test --workspace`)
- [x] 1.2 Add `VK_WORKSPACE_CLEANUP_INTERVAL_SECS` to control cleanup loop interval with sane defaults/clamps. (verify: `cargo test --workspace`)
- [x] 1.3 Thread an explicit cutoff into `Workspace::find_expired_for_cleanup` to decouple policy from storage logic. (verify: `cargo test --workspace`)

## 2. Generated env reference

- [x] 2.1 Add `scripts/generate-env-docs.js` and `pnpm run generate-env-docs{,:check}`. (verify: `pnpm run generate-env-docs:check`)
- [x] 2.2 Generate `docs/env.gen.md` and link it from `docs/operations.md`. (verify: `pnpm run generate-env-docs:check`)
- [x] 2.3 Enforce env-doc generation in CI. (verify: `.github/workflows/test.yml` includes `npm run generate-env-docs:check`)

