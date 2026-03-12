## 1. Backend: Structured Plan Detection (Recommended)

- [ ] 1.1 Add a `MilestonePlanDetectionResult` (found/not_found/invalid/unsupported) type in Rust and include it in TS generation. Verification: `pnpm run generate-types`
- [ ] 1.2 Implement a read-only API endpoint to detect the latest `milestone-plan-v1` payload from a guide `session_id` (or `attempt_id`) and return `MilestonePlanDetectionResult`. Verification: `cargo test -p server`
- [ ] 1.3 Add unit tests covering fenced vs embedded extraction, not-found, invalid JSON, and unsupported schema. Verification: `cargo test -p server milestone_planning`

## 2. Frontend: Guided Planner UX (Hide Raw JSON)

- [ ] 2.1 Rename milestone panel toggles to reduce ambiguity (`Planner` vs `Details`) and wire through i18n. Verification: `pnpm run check`
- [ ] 2.2 Redesign `MilestonePlanPanel` so the default UX is guide-driven: start/reuse guide attempt, auto-detect latest plan, and offer Preview/Apply without manual JSON editing. Verification: manual smoke check in dev
- [ ] 2.3 Add a gated "Advanced / Debug" disclosure to view/copy (and optionally import) the raw plan payload for troubleshooting, without making it the primary surface. Verification: advanced section is collapsed by default; `pnpm run lint`
- [ ] 2.4 Update planner copy to explain the operator flow (generate -> preview -> apply) and error states (no plan detected / invalid plan). Verification: manual smoke check + i18n keys exist

## 3. E2E & Regression Coverage

- [ ] 3.1 Update `e2e/milestone-planning.spec.ts` to match the new guided UX (and use the gated debug import only as a test harness fallback). Verification: `pnpm run e2e:test`
- [ ] 3.2 Add/adjust an e2e assertion that the raw JSON textarea is not visible in the default planner view (advanced disclosure remains closed). Verification: `pnpm run e2e:test`

