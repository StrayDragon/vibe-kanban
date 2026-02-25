# Change: Refactor deployment maintainability boundaries

## Why
Current server composition exposes too many subsystems through broad interfaces and large initializers, increasing coupling and making testing and future evolution harder.

## What Changes
- Introduce narrower service interfaces for route-layer dependencies.
- Decouple route state from concrete deployment implementation where feasible.
- Split monolithic deployment initialization into composable phases.
- Reduce repeated model-loader boilerplate with shared loading helpers.

## Impact
- Affected specs: `deployment-composition`
- Affected code: `crates/deployment/src/lib.rs`, `crates/server/src/routes/*.rs`, `crates/server/src/middleware/model_loaders.rs`, `crates/local-deployment/src/lib.rs`
- Out of scope: product feature changes.
