## Why

When projects grow, many tasks share similar titles (e.g., "Refactor X",
"Fix Y"). Relying on title-only search makes it hard to quickly locate a
specific card during reviews and coordination.

Upstream Vibe Kanban added the ability to search kanban issues by a short ID and
numeric number. We can deliver the same "fast targeting" capability for tasks
in our kanban.

## What Changes

- Expose a stable numeric task identifier (`number`) and a short ID (`short_id`)
  alongside the existing UUID `id`.
- Extend the Project Kanban search to match by:
  - `#<number>` / `<number>`
  - `<short_id>` / UUID prefix (case-insensitive)
  - existing title/description matching
- Display the numeric identifier in the kanban card UI to make referencing easy.

## Capabilities

### New Capabilities

- `kanban-id-search`: Users can search and reference tasks by number or short
  ID, not only by title/description.

### Modified Capabilities

<!-- None -->

## Impact

- Backend task model + TS type generation (adds `number` and `short_id` fields).
- Frontend kanban filtering logic and task card rendering.

## Goals / Non-Goals

**Goals:**
- Make task lookup fast and unambiguous.
- Keep the change backwards compatible (only additive fields).

**Non-Goals:**
- No server-side full-text search or indexing changes in this iteration.
- No per-project renumbering scheme (use existing stable identifiers).

## Risks

- Exposing internal numeric IDs might be confusing → display with a clear prefix
  (e.g., `#123`) and keep UUID as the canonical ID for APIs.

## Verification

- Create multiple tasks and verify the kanban search matches by:
  - `#<number>` and `<number>`
  - UUID prefix and `short_id`
  - title/description (existing behavior)
- `pnpm run generate-types` + `pnpm run check` + `cargo test --workspace`.

