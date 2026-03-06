## Why

The Rust workspace has already improved its low-level boundaries, but feature work still converges on a few broad crates: `server`, `services`, `deployment`, and `local-deployment`. Those crates currently mix composition, domain coordination, transport concerns, and blocking infrastructure work, which increases rebuild surface, makes async blocking mistakes easier, and forces unrelated changes to collide in the same files.

This change defines a new workspace shape optimized for parallel development: capability crates for business ownership, platform crates for shared foundations, and a thin runtime composition layer. The goal is to make crate ownership obvious before more MCP/task/runtime work accumulates on the current broad hubs.

## What Changes

- **BREAKING** Reorganize `crates/` around capability crates plus platform crates.
- **BREAKING** Replace the broad `deployment` / `local-deployment` service-locator pattern with a dedicated `app-runtime` composition root.
- **BREAKING** Split `services` into focused capability crates with explicit ownership boundaries.
- Re-scope `executors` into a registry/facade crate and formalize protocol/core/provider layering for executor implementations.
- Extend crate-boundary rules so transport adapters, blocking git/filesystem work, and runtime composition each have explicit homes.
- Preserve external HTTP/MCP behavior while restructuring internal crate dependencies.

Initial capability ownership in scope for this change:

| Target crate | Modules / responsibilities moved into it |
| --- | --- |
| `repos` | `project`, `repo`, `git`, `git/cli`, `worktree_manager`, `filesystem`, `filesystem_watcher`, `file_search_cache`, `file_ranker`, `workspace_manager` |
| `tasks` | `approvals`, `approvals/executor_approvals`, task/attempt coordination, archived kanban flows, idempotent create/start orchestration |
| `execution` | `container`, `diff_stream`, `queued_message`, `image`, `github`, `pr_monitor`, runtime session/container ownership, execution-log runtime coordination |
| `config` | `config/*`, runtime budget/config loading now in `cache_budget`, executor profile/config assembly |
| `events` | `events/*`, activity feed publication, event patch/stream shaping |
| `app-runtime` | startup sequencing, shutdown wiring, background jobs, `notification`, and service construction currently in `deployment` / `local-deployment` |

Workspace shape comparison:

```text
Before
crates/
├── server
├── services
├── deployment
├── local-deployment
├── executors
│   ├── registry
│   ├── shared runtime helpers
│   └── provider-facing glue
├── executors-core
├── executor-*
├── db
├── logs-*
└── utils-*

After
crates/
├── app-runtime
├── server
├── repos
├── tasks
├── execution
├── config
├── events
├── executors
│   └── registry / facade only
├── executors-core
├── executor-*
├── db
├── logs-*
└── utils-*

Removed from the main runtime path:
- services
- deployment
- local-deployment
```

## Goals

- Enable parallel backend development by assigning stable ownership to capability crates.
- Shrink the change surface of `server` and remove broad runtime/service-locator coupling.
- Make blocking git/filesystem work impossible to call accidentally from arbitrary async call sites.
- Keep executor provider implementations swappable without coupling them to server/runtime crates.
- Enforce the new dependency graph with auditable rules.

## Non-goals

- Changing HTTP routes, MCP tool schemas, or frontend behavior as part of this refactor.
- Reworking cache budgets, log retention policy, or other runtime tuning beyond boundary ownership.
- Keeping compatibility shims for old crate/module entrypoints.
- Introducing multi-step compatibility migrations for internal Rust call sites.

## Risks

- Large crate moves can create merge conflicts and temporarily slow reviews.
- Incorrect domain assignment can create new cyclic dependencies if the dependency graph is not enforced early.
- Startup/runtime wiring may regress if composition and background jobs are moved without a clear sequence.

## Verification

- `cargo check --workspace`
- `cargo test --workspace`
- `pnpm run backend:check`
- Boundary checks proving `server` no longer depends on broad service-locator crates and provider crates remain isolated from adapter crates.

## Capabilities

### New Capabilities
- `executor-runtime-layering`: define the registry/core/provider layering for executor crates so implementations remain swappable and transport-agnostic.

### Modified Capabilities
- `crate-boundaries`: add capability ownership rules, async-safe blocking boundaries, and restrictions against catch-all composition/service crates.
- `deployment-composition`: replace broad deployment facades with a dedicated runtime composition root and narrow domain injection into transport adapters.

## Impact

- **Code:** `crates/server`, `crates/services`, `crates/deployment`, `crates/local-deployment`, `crates/executors`, `crates/executors-core`, `crates/executor-*`, and new capability/runtime crates.
- **APIs:** no intended HTTP/MCP schema changes; internal Rust wiring and crate imports are intentionally breaking.
- **Dependencies:** new workspace members for capability crates and `app-runtime`; removal of broad cross-cutting dependencies from transport crates.
- **Systems:** startup composition, background job registration, executor discovery/selection, and CI boundary checks.
- **Ownership:** route groups and MCP handlers will resolve through capability-specific entrypoints instead of `services` and deployment-wide getters.
