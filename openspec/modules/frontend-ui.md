# Module: Frontend UI

## Goals
- Ensure loading states are accurate and recoverable.
- Keep task/attempt UI responsive to backend changes.
- Keep Task Group UX editable and promptable.

## In Scope
- useConversationHistory loading behavior.
- Task Group instructions surface in the UI.
- Minimal UI cues for auth failures (if token mode enabled).

## Out of Scope / Right Boundary
- Large UI redesign.
- New visual system or component library.
- Full user/login experience.

## Design Summary
- Fix loading overlay emit order so empty process lists clear loading.
- Provide Task Group node instructions editing in TaskGroupWorkflow.
- Keep UX changes minimal and localized.

## Testing
- Vitest tests for useConversationHistory.
- Manual sanity checks for Task Group edit flow.
