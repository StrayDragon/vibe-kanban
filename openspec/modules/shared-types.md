# Module: Shared Types

## Goals
- Keep Rust and TypeScript types in sync via generation.
- Prevent manual drift in shared/types.ts.

## In Scope
- Update Rust types and re-run type generation.
- Ensure new fields are exported where required.

## Out of Scope / Right Boundary
- Manual edits to shared/types.ts.
- Removing existing fields without compatibility plan.

## Design Summary
- All type changes originate in Rust.
- Use pnpm run generate-types after Rust changes.

## Testing
- pnpm run generate-types -- --check (or generate-types:check if available).
