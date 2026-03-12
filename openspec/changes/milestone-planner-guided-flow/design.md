## Context

Milestone planning currently exposes an internal machine format (`MilestonePlanV1` JSON) as a primary UX surface via a large textarea ("Plan input"). Although this format is valuable as a stable contract between the Guide agent and the system (preview/apply), it is not a good operator experience:

- Most users do not understand the schema, and copy/paste + JSON errors become the dominant failure mode.
- The UI already has the correct abstraction boundaries: a Guide attempt can produce a canonical `milestone-plan-v1` payload, and the server can preview/apply it atomically.
- The UX should emphasize conversation + deterministic diffs, not raw payload manipulation.

The system already supports:

- `MilestonePlanV1` schema + preview/apply endpoints (`milestone-planning` capability).
- A Guide attempt flow (prompt preset `milestone_planning`) that instructs the agent to emit a fenced `milestone-plan-v1` block.
- Client-side extraction heuristics that scan session message summaries/prompts for a plan block.

## Goals / Non-Goals

**Goals:**

- Remove the raw JSON textarea from the default (human) milestone planner experience.
- Make “Guide -> detect -> preview -> apply” the primary flow with minimal operator steps.
- Provide a clear, human-friendly preview summary before apply (metadata changes, tasks to create/link, node/edge diffs).
- Preserve the existing `MilestonePlanV1` contract as the internal machine format used between agent output and server preview/apply.
- Provide a gated way to view/copy the raw plan payload for debugging without making it the primary UX.

**Non-Goals:**

- Replacing the plan schema or changing preview/apply semantics.
- Building a full form-based graph editor for plans (nodes/edges editing) inside the planner panel.
- Making milestone planning fully autonomous (auto-apply without operator confirmation).

## Decisions

### 1) Default planner UX is guided; raw payload is hidden

The milestone “Planner” surface will:

- Start (or reuse) the Guide attempt.
- Detect the latest plan payload automatically.
- Offer Preview/Apply actions without manual copy/paste.

Raw plan payload input remains available only as a debug affordance (development-only or explicit "internal tools" gate).

**Alternatives considered:**

- Keep the textarea but label it "Advanced": still too prominent and continues to anchor user behavior around JSON editing.
- Remove all raw payload visibility entirely: makes debugging and support harder. A gated "view/copy" is a pragmatic compromise.

### 2) Prefer server-side plan detection/extraction (optional, recommended)

Client-side extraction exists today, but centralizing detection has benefits:

- Single implementation of extraction + validation logic across UI and potential future clients.
- More stable error model (structured "not found" vs "invalid" vs "unsupported schema").
- Easier testability at the API boundary.

Proposed shape:

- Add a small API endpoint that accepts `session_id` (or `attempt_id`) and returns a `MilestonePlanDetectionResult`:
  - `status`: `found | not_found | invalid | unsupported`
  - `plan`: `MilestonePlanV1 | null`
  - `extracted_from`: `fenced | embedded | null`
  - `source_turn_id`: string | null
  - `error`: string | null

The UI then becomes a thin consumer: “Fetch latest detected plan -> Preview -> Apply”.

**Alternatives considered:**

- Keep detection in the UI: fastest change, but duplicates parsing across clients and makes error handling less consistent. If we choose this alternative initially, we should still structure the UI so swapping to server-side detection later is non-breaking.

### 3) Reduce “Plan vs Plan” naming ambiguity via i18n

We should avoid having multiple unrelated UI elements called "Plan". Proposed naming:

- Panel toggle: `Planner` / `Details` (instead of `Plan` / `Details`).
- Planner section title: `Plan` or `Draft` (but not `Plan input`).
- Debug affordance: `Show raw plan payload`.

All labels should be wired through i18n keys to keep terminology consistent.

## Risks / Trade-offs

- **Removing manual JSON input reduces recovery options** → Provide a gated debug view/copy of the extracted payload; keep Preview/Apply deterministic so operators can iterate via Guide follow-ups instead of editing JSON.
- **Plan detection can fail if agents do not emit canonical fences** → Keep the Guide preset strict (`milestone-plan-v1` fence) and surface detection errors with actionable guidance (e.g. “Ask the guide to re-emit the plan block”).
- **Server-side detection adds API surface area** → Keep the endpoint narrow and read-only; reuse existing session message storage; add unit tests to lock extraction behavior.

## Migration Plan

- Frontend-only changes can ship without data migration.
- If we add server-side plan detection:
  - Add read-only endpoint + TS types generation if the type is shared.
  - Update the UI to consume the endpoint (behind a feature flag if we want an incremental rollout).
  - Update e2e tests to validate the guided flow end-to-end.

## Open Questions

- Should the debug affordance be strictly dev-only, or controlled by a persisted “internal tools” user setting?
- Should we cache the last detected plan (e.g. per milestone) to avoid re-scanning session messages, or keep it ephemeral for simplicity?

