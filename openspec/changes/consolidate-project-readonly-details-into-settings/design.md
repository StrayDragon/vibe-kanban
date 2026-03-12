## Context

`ProjectDetail` currently represents a second project-level UI surface with mostly read-only information:
- project ID
- created / updated timestamps
- project execution summary
- lifecycle-hook configuration summary
- latest recorded lifecycle-hook outcome

In the live product flow, operators already manage projects from `Settings > Projects`, and `/projects/:projectId` redirects to the task view instead of exposing `ProjectDetail`. That means the old detail page adds maintenance cost without serving as an intentional user workflow.

## Goals / Non-Goals

**Goals:**
- Consolidate project-level metadata and hook diagnostics into the selected-project settings experience.
- Keep read-only information compact so editable settings remain the primary focus.
- Remove obsolete detail-page code once settings parity exists.

**Non-Goals:**
- Introduce a new project overview route.
- Change server DTOs or config persistence unless parity reveals a missing field.
- Redesign unrelated settings sections.
- Change task/attempt navigation or kanban behavior.

## Decisions

### 1. Treat `Settings > Projects` as the canonical project summary surface

The selected-project settings page already owns project configuration. This change extends that same surface with the remaining read-only information operators still need, rather than preserving a second view-only page.

### 2. Add compact read-only metadata near the existing project settings flow

Recommended readonly fields:
- project ID
- created timestamp
- last modified timestamp

These should appear in a compact summary card or footer section within the selected-project view so they remain discoverable without competing with editable controls.

Fields that should stay where they already are instead of being duplicated:
- execution mode
- scheduler concurrency / retries

Those values are already editable inside project settings and do not need a second read-only presentation.

### 3. Keep lifecycle-hook latest-run visibility in the existing lifecycle-hooks section

The existing lifecycle-hooks editor in project settings is the natural place to show:
- whether a hook is configured
- the latest task that produced a hook result
- the latest hook outcome summary
- concise loading / empty states

The summary should reuse the compact hook-summary presentation rather than the larger task/attempt detail card.

Implementation note: keep this frontend-scoped by reusing the existing `ProjectDetail` approach for now (scan a small set of recently updated tasks, fetch attempts, select the most recent workspace hook outcome). This keeps backend untouched while achieving parity.

**Performance note (recommended): make the "latest hook outcome" query on-demand.**

The existing approach can be expensive (multiple `attemptsApi.getAllWithSessions` calls). To keep `Settings > Projects` responsive:
- the hook outcome summary SHOULD be behind an expander or an explicit "Load latest hook run" action
- the query SHOULD be disabled until the user expands/requests the summary
- the UI MUST still provide clean empty/loading/error states

This avoids surprising background network load when an operator opens settings just to edit configuration.

### 4. Remove the standalone detail-only page after parity is reached

Once settings provides the needed readonly metadata and hook summary, the following should be removed or retired:
- `ProjectDetail`
- `Projects` page wrapper if it no longer serves a routed surface
- any temporary or legacy routes/imports that only existed for that detail view

Shared hook-summary components should be retained only if they continue to serve active surfaces.

### 5. Prefer reuse over new view models

Project settings already has selected-project state and lifecycle-hook config state. Implementation should reuse that context and only add the minimal query logic needed for the latest hook outcome if it is not already present on the selected project payload.

## Migration Plan

- Add the compact readonly summary to the selected project settings page.
- Port the latest hook outcome summary into the lifecycle-hooks section.
- Remove obsolete project-detail routes/components once parity is visible.
- Keep backend untouched unless the settings page lacks fields already available elsewhere.

## Risks / Trade-offs

- A naive port of the old detail content can make settings visually dense.
- Removing the old detail page too early could hide metadata if settings parity is incomplete.
- If lifecycle-hook latest-run data still requires extra task/attempt reads, the settings page may need careful loading states to avoid jank.
- If the hook outcome query is executed automatically on initial render, it can create unnecessary network load; keep it user-initiated unless/until a lightweight backend endpoint exists.

## Verification

- `pnpm run frontend:check`
- `pnpm run frontend:lint`
- Browser smoke check in `Settings > Projects` showing:
  - a project with no lifecycle-hook runs yet
  - a project with a recorded latest hook failure or success
- A code search confirming the deprecated detail route/components are no longer part of active navigation.
