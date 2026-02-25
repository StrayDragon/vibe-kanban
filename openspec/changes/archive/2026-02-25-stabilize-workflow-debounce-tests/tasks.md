## 1. Implementation
- [x] 1.1 Migrate debounce tests to `vi.useFakeTimers()` and deterministic timer advancement.
- [x] 1.2 Remove fixed `setTimeout` sleeps from affected tests.
- [x] 1.3 Add guard assertions using `waitFor` where async render scheduling is involved.

## 2. Verification
- [x] 2.1 `pnpm -C frontend run test`
- [x] 2.2 `openspec validate stabilize-workflow-debounce-tests --strict`
