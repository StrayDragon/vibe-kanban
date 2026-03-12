## Why

VK already uses `Settings > Projects` as the primary surface for project-level configuration, but a separate `ProjectDetail` page still exists in the codebase with overlapping read-only metadata and lifecycle-hook diagnostics. That split creates stale UI code, duplicates maintenance work, and makes hook visibility inconsistent between the active settings workflow and an effectively unused detail page.

Today `ProjectDetail` is effectively unreachable in the product flow: `/projects` and `/projects/:projectId` redirect to task views, leaving a large chunk of UI code as dead or near-dead code that still needs to be maintained.

## What Changes

- Move the remaining useful read-only project metadata into the selected-project settings experience.
- Make the existing lifecycle-hook settings area the canonical place for latest hook outcome visibility.
- Remove or retire the obsolete standalone project-detail surface after the settings page reaches parity.
- Keep the migration strictly frontend-scoped unless a missing API field is discovered during implementation.
- Keep settings fast: any "latest hook outcome" queries that require scanning tasks/attempts SHOULD be lazily loaded (for example behind an expander) so opening project settings does not trigger heavy background fetches by default.

## Capabilities

### New Capabilities
- `project-settings-summary`: a canonical project settings summary surface that combines editable settings with essential read-only metadata and hook diagnostics.

### Modified Capabilities
- None.

## Impact

- Frontend: `frontend/src/pages/settings/ProjectSettings/ProjectSettings.tsx`, related hook summary components, and route/import cleanup in `frontend/src/app/AppRouter.tsx`.
- UX: project operators get one consistent place to review project metadata, lifecycle-hook configuration, and latest hook outcome.
- Maintenance: removes dead or near-dead UI code paths that no longer participate in the main product flow.

## Reviewer Guide

- This change is intentionally narrow: it does not introduce a new project page, backend API, or data model.
- The main acceptance bar is consolidation: all still-useful read-only project information is visible from the active settings flow, and the deprecated detail-only surface can be removed.
- Execution mode editing stays where it already is; this proposal is about summary/read-only parity, not reworking scheduler controls.
- Reviewers should treat `ProjectDetail` / `Projects` as deprecated implementation detail: the goal is to delete that surface once settings parity is achieved, not to preserve it.

## Goals

- Make project settings the single human-facing home for project metadata and lifecycle-hook diagnostics.
- Remove duplicate or abandoned project-detail UI code once settings parity is achieved.
- Keep the hook summary visually compact and consistent with the current human-first settings UX.

## Non-goals

- Reintroducing a standalone project dashboard page.
- Changing backend config semantics or adding new project persistence fields.
- Redesigning all project settings sections in the same change.
- Moving task-, attempt-, or review-specific diagnostics into project settings.

## Risks

- Project settings can become visually heavy if read-only metadata is not compact.
- Hook summary duplication can reappear if old components are removed inconsistently.
- Removing the old detail surface can break hidden links if any code still depends on it.
- A naive port of the old hook "latest outcome" logic can add too many network calls on initial settings load; this should be mitigated with on-demand loading and clear empty/loading states.

## Verification

- `pnpm run frontend:check`
- `pnpm run frontend:lint`
- Manual browser smoke check at `Settings > Projects` for one plain project and one project with recorded lifecycle-hook activity.
  - Confirm the project settings page renders without triggering the expensive "scan tasks + fetch attempts" hook outcome query until the user intentionally expands or requests it.
