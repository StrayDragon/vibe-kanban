# Task: T-008 Format Cleanup

## Background / Motivation
- Issue: P2-FMT-01
- Evidence: cargo fmt --check fails.

## Scope
### In Scope
- Run cargo fmt --all and commit formatting changes.

### Out of Scope / Right Boundary
- Any functional code changes.

## Design
- Pure formatting only.

## Change List
- Files touched by rustfmt.

## Acceptance Criteria
- cargo fmt --all -- --check passes.

## Risks & Rollback
- Low risk; revert formatting commit if needed.

## Effort Estimate
- 0.5 day.

## Acceptance Scripts
```bash
cargo fmt --all -- --check
```
Expected:
- No formatting diffs.
