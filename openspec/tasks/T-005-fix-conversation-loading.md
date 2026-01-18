# Task: T-005 Fix Conversation Loading State

## Background / Motivation
- Issue: P1-FE-01
- Evidence: frontend test fails in frontend/src/hooks/UseConversationHistory.test.tsx.

## Scope
### In Scope
- Fix loading emit order in useConversationHistory.
- Update test if needed to match correct behavior.

### Out of Scope / Right Boundary
- Any redesign of conversation UI.

## Design
### Proposed
- Ensure that when executionProcesses is empty and loading is false, emit loading=false and do not overwrite it later in the same render cycle.
- Keep logic localized to useConversationHistory.

## Change List
- frontend/src/hooks/useConversationHistory.ts: adjust effect order/guards.
- frontend/src/hooks/UseConversationHistory.test.tsx: update assertions if needed.

## Acceptance Criteria
- pnpm -C frontend run test passes.
- The test "clears loading when there are no execution processes" passes.

## Risks & Rollback
- Low risk, localized change.
- Rollback by reverting the hook change.

## Effort Estimate
- 0.5 day.

## Acceptance Scripts
```bash
pnpm -C frontend run test
```
Expected:
- UseConversationHistory.test.tsx passes.
