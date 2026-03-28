# frontend-performance-guardrails Specification (Delta)

## ADDED Requirements

### Requirement: Conversation history derivations are incremental and stable
When consuming realtime execution-process updates, the frontend SHALL avoid full rebuilds of derived conversation history collections when updates are localized to a small set of execution process ids.

For conversation history derived from entry-indexed log pages:
- Per-process entry lists MUST remain sorted by `entry_index` ascending.
- For id-local updates affecting a single execution process, the frontend MUST apply incremental derivation updates scoped to that process, rather than re-sorting and re-flattening all processes on every update.
- Unaffected per-process entry lists SHOULD preserve referential equality across localized updates to maximize React memoization efficiency.

#### Scenario: Derived conversation entries remain correctly ordered
- **WHEN** a conversation view receives a sequence of localized execution-process entry updates
- **THEN** per-process entries remain sorted by `entry_index` ascending

#### Scenario: Unaffected process lists keep stable references on localized updates
- **WHEN** a localized update modifies entries for one execution process
- **THEN** the derived entry lists for other execution processes remain referentially stable

