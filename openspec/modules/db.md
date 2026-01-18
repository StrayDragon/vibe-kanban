# Module: Database Consistency

## Goals
- Preserve consistency for task/attempt/workspace relationships.
- Prevent orphaned rows from partial workflows.

## In Scope
- Transaction usage around multi-step writes.
- Cleanup behavior for failed attempts.
- Minimal schema adjustments only if required for consistency.

## Out of Scope / Right Boundary
- Multi-tenant schema changes.
- Database engine migration.
- Large indexing or performance changes.

## Design Summary
- Use SeaORM transactions for create/start paths.
- If new config fields are added, bump config version and add migration if needed.
- Avoid schema changes unless a consistency invariant cannot be met otherwise.

## Testing
- Use sqlite in-memory tests to validate rollback behavior.
