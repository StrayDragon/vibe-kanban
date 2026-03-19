## Why

We have multiple entry points that select an agent + configuration ("model"):
onboarding, Settings → Agents, task/attempt creation dialogs, and milestone
workflow defaults. Today, the default executor selection is not fully unified
across these flows, and the data shape is inconsistent (e.g. some paths store
`variant: "DEFAULT"` while others use `null`).

This causes confusing UX ("why did it pick this model?"), makes defaults harder
to reason about, and can break UI highlighting / persistence.

## What Changes

- Define and enforce a single canonical representation for
  `ExecutorProfileId.variant`:
  - `null` (missing) means DEFAULT
  - `"DEFAULT"` (any case) is normalized to `null` at boundaries
- Unify default executor profile resolution for new attempts:
  - locked milestone node profile > user selection > last used profile for the
    task (including variant) > user system default
- Expose the last used executor profile (executor + variant) for an attempt so
  the UI can default correctly without guessing.
- Reuse the same selector behavior across onboarding + dialogs (avoid bespoke
  "variant dropdown" logic).

## Capabilities

### New Capabilities

- `executor-profile-defaulting`: Consistent default executor profile selection
  and canonical profile ID representation across UI + API flows.

### Modified Capabilities

<!-- None -->

## Impact

- Backend: attempt/session summary DTOs used by attempt creation UI.
- Config: `crates/config/src/schema.rs` normalization for executor profile
  variant.
- Frontend: onboarding profile picker and Create Attempt defaulting logic.
- Types: `shared/types.ts` regeneration after Rust DTO changes (if applicable).

## Goals / Non-Goals

**Goals:**
- Make "DEFAULT" behave identically everywhere (stored as `null`).
- Make default selection predictable and consistent across flows.

**Non-Goals:**
- No new executor/model discovery UX.
- No change to follow-up executor switching semantics (variant-only remains).

## Risks

- Extra DB/API lookups to derive "last used profile" → keep it scoped to the
  latest coding-agent process and cache/limit where needed.

## Verification

- Existing configs with `variant: "DEFAULT"` load and are rewritten to
  `variant: null` after save.
- Create Attempt dialog preselects the same profile as the last coding-agent run
  (including variant) when available.
- `pnpm run check`, `pnpm run lint`, `pnpm run backend:check`,
  `cargo test --workspace`.

