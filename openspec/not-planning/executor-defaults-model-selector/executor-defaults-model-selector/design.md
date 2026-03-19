## Context

We represent the selected agent + configuration as an `ExecutorProfileId`
(`{ executor, variant }`), where `variant` is optional and should represent the
DEFAULT configuration when missing.

Today there are two classes of issues:

1) **Representation drift**

- `frontend/src/components/dialogs/global/OnboardingDialog.tsx` builds the
  variant dropdown directly from the profile keys and can store
  `variant = "DEFAULT"`.
- `frontend/src/components/tasks/ConfigSelector.tsx` treats DEFAULT as
  `variant = null`.
- `crates/config/src/schema.rs` normalizes empty variants to `null`, but does
  not normalize `"DEFAULT"` → `null`.

This can lead to inconsistent highlighting and makes it harder to reason about
what the stored value means.

2) **Defaulting drift**

- `frontend/src/components/dialogs/tasks/CreateAttemptDialog.tsx` tries to pick a
  sensible default based on the latest attempt, but it only has
  `attempt.session.executor` and cannot reliably recover the *variant* that was
  used ("TaskAttempt doesn't store it").
- The backend already records the full `ExecutorProfileId` in execution process
  actions (`CodingAgentInitialRequest` / `CodingAgentFollowUpRequest`), and it
  has helper APIs like
  `ExecutionProcess::latest_executor_profile_for_session(...)`.

The end result is that the "Model Selector" (what the user thinks they picked)
and the "Default Executor Flow" (what the system actually defaults to) can
diverge across surfaces.

## Goals / Non-Goals

**Goals:**
- Canonicalize DEFAULT representation: store DEFAULT as `variant = null`.
- Provide a single, well-documented default executor profile resolution order
  for new attempts.
- Remove variant "guessing" by exposing the last used coding-agent profile to
  the Create Attempt dialog.

**Non-Goals:**
- No new model discovery UX (fetching remote model lists, etc.).
- No behavior change that allows switching executors mid-session for follow-ups
  (variant-only remains).
- No broad profiles format refactor (keep `crates/executors/default_profiles.json`
  semantics as-is).

## Decisions

### Decision: `variant = null` is the canonical DEFAULT representation

We will treat the DEFAULT configuration as `variant = null` across config, API,
and UI.

At boundaries we will normalize:
- empty / whitespace variant → `null` (already done)
- `"DEFAULT"` (case-insensitive, trimmed) → `null` (new)

**Alternative:** Keep `"DEFAULT"` as a valid stored value and update all UI
paths. Rejected because `null` is already the prevalent representation and is
friendlier to serde (`skip_serializing_if`).

### Decision: Default attempt selection uses an explicit precedence order

The Create Attempt UI will use the following precedence order:

1. Milestone node override (locked)
2. User selection in the dialog
3. Last used coding-agent `executor_profile_id` (including variant)
4. User system default `config.executor_profile`

**Alternative:** Always use `config.executor_profile`. Rejected because it loses
the "repeat the last run" workflow that many users expect.

### Decision: Expose last used coding-agent profile via API (derived, not stored)

We will add an optional field to the attempt/session summary payload used by the
Create Attempt dialog to carry the last used coding-agent `ExecutorProfileId`.

Initial approach: derive it from the latest coding-agent execution process for
the session/workspace.

**Alternative (DB change):** Persist `executor_profile_id` (or a variant column)
on `sessions` at creation time. Deferred because it requires a DB migration and
careful backfill semantics; we can revisit if API derivation proves too costly.

## Risks / Trade-offs

- **[Extra DB work]** deriving per-attempt last-used profile can become an N+1
  query. Mitigation: keep it scoped to "latest coding-agent process only" and
  consider computing it only for the latest attempt when needed.
- **[Legacy config values]** existing configs may already store `"DEFAULT"`.
  Mitigation: normalize in `Config::normalized()` and add a unit test.

## Migration Plan

1. Config: normalize `"DEFAULT"` → `null` for `executor_profile.variant`.
2. Frontend: update onboarding selector behavior to store `null` for DEFAULT.
3. Backend: extend attempt/session summary DTOs to include last used
   `executor_profile_id` (derived from execution processes).
4. Frontend: update Create Attempt defaulting to use the new field (no more
   guessing).
5. Regenerate TypeScript types if Rust DTOs change and run checks/tests.

## Open Questions

- Should defaulting prefer the *last used profile* or the *system default* when
  they differ? (Current default: prefer last used.)
- Should we later persist `executor_profile_id` on `sessions` for O(1) reads?
  (Default: **no**, only if profiling shows it matters.)

