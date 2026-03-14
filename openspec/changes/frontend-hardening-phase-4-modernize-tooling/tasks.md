## 1. Baseline

- [x] 1.1 Capture current versions and upgrade candidates (`pnpm -C frontend outdated`) and record the intended “safe lane” targets for this phase.
- [x] 1.2 Run baseline verification: `pnpm -C frontend run check`, `pnpm -C frontend run lint`, `pnpm -C frontend run build`, `pnpm run e2e:just-run`, and `pnpm -C frontend audit --prod`.

## 2. Safe Lane Upgrades (Incremental)

- [x] 2.1 Upgrade build tooling (Vite and related plugins) in a small batch and verify the full gate (`check/lint/build/e2e/audit`).
- [x] 2.2 Upgrade test tooling (Vitest and related deps) and verify tests still pass (`pnpm -C frontend run test` + `pnpm run e2e:just-run`).
- [x] 2.3 Upgrade lint tooling (ESLint + plugins) and verify `pnpm -C frontend run lint` still passes.
- [x] 2.4 Upgrade state/tooling libs with bounded migrations (e.g., Zustand major) and verify key flows + e2e.

## 3. High-Risk Lane Spikes (Go/No-Go)

- [x] 3.1 Tailwind major spike: attempt upgrade in isolation, document required config changes and UI drift risk, and decide go/no-go based on e2e + manual smoke.
- [x] 3.2 React ecosystem major spike (React 19 / Router 7): document compatibility matrix for key deps and decide go/no-go based on verified build + e2e.

## 4. Finalization

- [x] 4.1 Ensure security baseline is preserved: `pnpm -C frontend audit --prod` has 0 HIGH/MODERATE.
- [x] 4.2 Document the upgraded toolchain versions and the repeatable upgrade/verification procedure.
