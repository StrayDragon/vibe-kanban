## Why

The current milestone planner UI exposes a raw JSON textarea ("Plan input") as a first-class surface. This is confusing for most operators, encourages copy/paste error handling, and makes milestone planning feel like an internal API rather than a guided workflow.

We already have the right contract: the Guide agent can emit a versioned `milestone-plan-v1` payload that the system can preview/apply. The UI should treat this payload as an internal machine format and only surface human-friendly controls and diffs.

## What Changes

- **BREAKING (UI)**: Remove the raw plan JSON textarea from the default milestone planner UI.
- Replace it with a guided flow that:
  - Starts (or reuses) a milestone Guide attempt.
  - Automatically detects the latest `milestone-plan-v1` payload from the Guide output.
  - Offers one-click **Preview** and **Apply** actions without requiring manual copy/paste.
  - Displays a human-friendly diff summary (metadata changes, tasks to create/link, node/edge changes).
- Add an "Advanced / Debug" affordance (development-only or explicitly gated) to **view/copy** the raw plan payload when needed for debugging, without making it the primary UX.
- Clarify naming to avoid “Plan vs Plan” confusion (e.g. "Planner" vs "Details", and "Plan input" becomes an internal/debug label only).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `milestone-planning`: Update the planner UX requirements so preview/apply can be driven directly from Guide output without exposing raw JSON as the primary interaction model.

## Impact

- Frontend: milestone workflow planning UI (`MilestoneWorkflow`, `MilestonePlanPanel`), i18n copy, and e2e flows around planning.
- Backend: may add a small extraction/validation helper endpoint to centralize plan detection (optional but recommended for consistency across clients).
- Testing: update or add e2e coverage for “generate plan -> detect -> preview -> apply” without manual JSON input.

