## 1. Implementation
- [x] 1.1 Remove the versioned config modules and define a single latest Config schema.
- [x] 1.2 Update config load/save to deserialize with defaults and persist only the latest schema.
- [x] 1.3 Add minimal serde aliases and a normalize step for non-breaking field/value adjustments.
- [x] 1.4 Replace migration tests with focused tests for defaults, alias handling, and fallback behavior.
- [x] 1.5 Regenerate or update shared type generation if required by the new schema.
