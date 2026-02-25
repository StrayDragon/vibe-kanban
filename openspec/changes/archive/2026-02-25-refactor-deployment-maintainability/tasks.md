## 1. Implementation
- [x] 1.1 Define focused route-facing service traits and adapters.
- [x] 1.2 Update route handlers to depend on narrowed state interfaces.
- [x] 1.3 Split `LocalDeployment::new` into staged builders/factories.
- [x] 1.4 Introduce reusable model-loading helper(s) to reduce duplication.
- [x] 1.5 Add unit/integration tests for new composition boundaries.

## 2. Verification
- [x] 2.1 `cargo test --workspace`
- [x] 2.2 `openspec validate refactor-deployment-maintainability --strict`
