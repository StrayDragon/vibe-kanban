## 0. Scope & Constraints
- Scope: log normalization resilience, log view stable identity, and the “no processes” conversation-history loading state.
- Non-goals: redesigning the log payload schema; changing endpoint URLs.

## 1. Frontend: conversation history loading
- [ ] 1.1 Ensure `useConversationHistory` sets `loading=false` when the process list is empty and loading is complete.
- [ ] 1.2 Add/extend tests in `frontend/src/hooks/UseConversationHistory.test.tsx` for:
  - empty list clears loading
  - still-loading does not clear

## 2. Backend: normalization resilience
- [ ] 2.1 Replace panic/unwrap paths in normalization with recoverable errors + `tracing` logs.
- [ ] 2.2 Emit explicit “normalization error” entries when a tool result/event cannot be matched to expected state, and continue streaming.
- [ ] 2.3 Ensure event patch/stream construction avoids `expect/unwrap` on high-risk inputs; log and continue when feasible.

## 3. Frontend: stable identity for log items
- [ ] 3.1 Use stable keys (`entryIndex` / `patchKey`) for raw + normalized log rendering.
- [ ] 3.2 Remove expensive equality checks based on `JSON.stringify` where possible; prefer memoization keyed by stable identity.
- [ ] 3.3 Ensure “prepend older history” preserves rendered item identity and scroll stability.

## 4. Tests
- [ ] 4.1 Backend: add a resilience test covering anomalous/out-of-order tool result sequences (no panic, emits error entry).
- [ ] 4.2 Frontend: add a log-view stability test (prepends preserve identity / avoids full re-render) if test harness exists.

## 5. Verification
- [ ] 5.1 `cargo test --workspace`
- [ ] 5.2 `pnpm -C frontend run test`
- [ ] 5.3 `pnpm -C frontend run check`
- [ ] 5.4 `pnpm -C frontend run lint`

## Acceptance Criteria
- Anomalous log sequences do not panic the server; the stream remains live and includes an error entry describing the anomaly.
- The conversation view does not remain stuck in loading state when there are no processes to load.
- Log items keep stable identity when older history is loaded and prepended.

