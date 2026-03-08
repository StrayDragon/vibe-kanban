## Context

The Kanban/task surfaces (`/tasks`, `/projects/:id/tasks`) are fed primarily by realtime WebSocket JSON Patch streams (`useAllTasks`, `useProjectTasks` via `useJsonPatchWsStream`). Task mutations (create/update/delete) are performed via HTTP (`tasksApi.*`) and the UI generally assumes the stream will reflect the mutation shortly after the API response resolves.

Observed failure mode: mutations succeed (HTTP 2xx), but the UI remains stale until the user navigates away and returns (forcing a new stream subscription + snapshot). This indicates at least one of:
- The stream misses updates intermittently (backend event emission or transport).
- The client fails to reconcile a subset of patches.
- The UI has no fallback resync path when the stream is stale.

Separately, a per-column “Add task” icon button exists in the Kanban header but is effectively non-interactive due to styling/hitbox issues. Project creation currently auto-creates on repo selection (single-step implicit flow), and project deletion confirmation does not include disambiguating identifiers.

This change focuses on:
- Reliability of task UI state after mutations.
- Safety and clarity of project lifecycle flows.
- Adding Playwright E2E coverage that asserts both “immediate UI update” and “reload consistency”.

## Goals / Non-Goals

**Goals:**
- After a successful task mutation, update the UI immediately (no required navigation to see the new state).
- Provide a deterministic resync path when realtime streams fall behind, without blanking the UI.
- Make per-column add-task interaction usable (mouse + keyboard) and easy to target in E2E tests.
- Make project create/delete flows explicit and safe (minimize irreversible mis-click risk).
- Add Playwright E2E tests covering the regression matrix and asserting “immediate update + reload consistency”.

**Non-Goals:**
- Replacing the realtime streaming model wholesale (unless required by stream correctness gaps discovered during implementation).
- A full-site i18n/a11y overhaul; changes are scoped to touched surfaces and a minimum baseline.
- Major visual redesign of Kanban layout, routing, or attempt panels.

## Decisions

### 1) Treat the stream as “eventually consistent”, and make mutations “immediately consistent”

Rationale: Users interact with mutations directly; the UI must reflect their action even if the stream update is delayed or lost.

Implementation approach (frontend-first):
- Introduce a small optimistic overlay layer for task collections used by `/tasks` and `/projects/:id/tasks`:
  - For status changes (drag/drop), immediately move the card between columns locally and keep it there while the mutation is in-flight.
  - For deletes, remove locally on success (or immediately if backend returns async/202 semantics), with rollback + toast on failure.
  - For edits that change status, apply the same “move now” behavior and reconcile fields when stream delivers the canonical version.
- Add a “soft resync” trigger for `useJsonPatchWsStream` consumers:
  - Force a reconnect (new snapshot) when a mutation succeeds but the stream does not reflect the expected state within a short window.
  - Preserve current UI state while reconnecting to avoid empty-column flicker.

Alternative considered: introduce REST list endpoints and make React Query the only source of truth (with WS as incremental updates). This is more robust long-term but is larger in scope; it can be a follow-up if stream reliability requires backend work anyway.

### 2) Fix the column add-task control via proper sizing + stable selector

Rationale: The per-column affordance is a key workflow accelerator, and E2E must be able to target it without relying on brittle CSS selectors.

Implementation approach:
- Use `Button`’s icon size conventions (e.g., `size="icon"` and explicit `h/w`), avoiding `h-0`/zero hitbox classes.
- Add `data-testid` on the header control (and optionally per-status variant like `data-testid="kanban-add-task-todo"`).
- Ensure keyboard accessibility is preserved (button is focusable, has `aria-label`, and triggers the same create-task modal as the global create button).

### 3) Replace implicit project creation with an explicit wizard

Rationale: “Click list item → side effect (create project)” is error-prone, especially with same-name projects and repos/worktrees.

Implementation approach:
- Replace `ProjectFormDialog`’s current auto-flow with a real dialog that:
  1) Selects or creates a repo (no project creation side-effects yet).
  2) Allows editing/confirming the project name (pre-filled from repo name).
  3) Shows the chosen repo path/ID and requires an explicit “Create project” confirmation.
- Add safety checks:
  - Detect temporary worktree paths (common patterns like `/worktrees/`, `/tmp/`, `.git/worktrees`) and block or require explicit confirmation.
  - If the project name already exists, force disambiguation in UI (show repo path/ID) and avoid ambiguous deletes.
- Update delete confirmations to show at minimum: project name + repo path (or repo IDs) to avoid same-name deletion mistakes.

Alternative considered: keep the one-step fast path but add a confirm dialog after repo click. Rejected because it still encourages accidental creation and provides poor disambiguation.

### 4) E2E suite asserts “immediate update + reload consistency”

Rationale: Many regressions only show up as stale UI state after a mutation or after a reload.

Implementation approach:
- Add Playwright specs that:
  - Perform a mutation (create/edit/status change/drag/delete).
  - Assert the UI updates immediately (DOM state changes without navigation).
  - `page.reload()` and re-assert consistency.
- Prefer role-based selectors + `data-testid` for critical controls (e.g., column add-task, delete confirmation).
- Keep tests deterministic via `scripts/run-e2e.js` (seeded fake agent, fixed config, isolated asset/repo dirs).

## Risks / Trade-offs

- [Optimistic UI divergence] → Mitigation: reconcile with stream updates; add resync fallback; use toasts for failures; keep optimistic layer minimal and scoped.
- [Resync causes jarring resets] → Mitigation: “soft reconnect” that preserves current state until new snapshot arrives; cap resync frequency.
- [Wizard adds friction] → Mitigation: prefill project name from repo; keep steps minimal; allow keyboard flow; provide clear affordances.
- [E2E flakiness] → Mitigation: stable selectors; deterministic seed; avoid arbitrary sleeps; add explicit wait conditions for navigation and UI transitions.

## Migration Plan

- Frontend-only by default:
  - Implement optimistic + resync mechanisms, column button fix, project wizard, and new E2E suite.
- If backend stream correctness issues are confirmed during implementation:
  - Patch server-side task event emission/stream snapshot behavior.
  - Keep API shapes stable; only add endpoints if necessary for robustness (future phase).
- Rollback strategy:
  - Optimistic/resync changes can be feature-flagged at the hook level if needed (optional).
  - Wizard can keep a “fast path” behind a flag temporarily if rollout friction is high.

## Open Questions

- Is the root cause primarily backend stream emission, client patch processing, or connection lifecycle? (Implementation should add lightweight logging/telemetry hooks to narrow this down during dev.)
- Do we want a long-term move to query-backed task lists with REST snapshots + WS incremental updates? (Out of scope for phase 1, but may become necessary if streams remain unreliable.)
