## Context

The workspace already has solid foundation crates (`executors-protocol`, `logs-protocol`, `logs-store`, `utils-*`), but the main runtime path still concentrates ownership in a small set of broad crates:

- `server` mixes transport adapters, MCP task orchestration, and broad route state wiring.
- `services` acts as a catch-all application layer for repos, tasks, execution, config, and events.
- `deployment` and `local-deployment` act as service locators plus runtime composition plus background lifecycle management.
- `executors` still combines registry/facade concerns with provider-facing runtime coupling.

This structure increases rebuild scope, makes async-safe infrastructure boundaries harder to enforce, and creates merge hotspots around `server`, `services`, and runtime startup. The requested end state is not “clean layering for its own sake”; it is a workspace shape that lets multiple engineers work in parallel with predictable ownership boundaries.

## Goals / Non-Goals

**Goals:**
- Establish a stable crate topology: platform crates, capability crates, and a thin composition/runtime crate.
- Remove the broad deployment façade and replace it with an explicit application runtime.
- Assign each major backend concern to a single domain crate so changes stop converging on `services`.
- Require blocking git/filesystem work to stay inside dedicated capability-owned blocking boundaries.
- Keep the external HTTP/MCP contract stable while internal crate ownership changes.
- Restructure the executor stack so provider implementations depend only on protocol/core layers.

**Non-Goals:**
- Redesigning HTTP/MCP APIs, UI flows, or persistence schemas solely for this refactor.
- Solving all cache/log/runtime tuning problems in the same change.
- Maintaining compatibility layers for old module paths or broad runtime facades.
- Changing config semantics unless runtime composition needs a versioned config update.

## Decisions

### 1) Adopt a hybrid workspace model: capability domains + platform foundations

The workspace will use three explicit crate families:

- **Platform crates**
  - `db`, `db-migration`
  - `executors-protocol`, `logs-protocol`
  - `logs-store`, `logs-axum`
  - `utils-*`
- **Capability crates**
  - `repos`
  - `tasks`
  - `execution`
  - `config`
  - `events`
- **Composition / adapter crates**
  - `app-runtime`
  - `server`
  - `review` (unchanged in this change)

This is intentionally not a pure clean-architecture split and not a pure feature-package split. The hybrid model keeps the low-level foundations shared while giving business ownership to capability crates. That is the best fit for parallel development in the current codebase.

**Alternatives considered**
- **Pure layered architecture (`api/application/domain/infrastructure`)**: rejected because it would force large conceptual migration cost onto a codebase that already has useful platform crate splits.
- **Pure vertical slices for everything**: rejected because protocol/persistence/log foundations are already shared and benefit from explicit platform ownership.
- **Keep current crates and only split files**: rejected because it would reduce file size but preserve the same broad dependency graph and merge hotspots.

Current vs target workspace tree:

```text
Before
crates/
├── server
│   ├── routes
│   ├── mcp
│   └── broad runtime state
├── services
│   ├── repos / filesystem
│   ├── tasks / approvals
│   ├── execution / container
│   ├── config
│   └── events
├── deployment
├── local-deployment
│   ├── runtime composition
│   └── local execution helpers
└── executors
    ├── registry
    ├── shared runtime helpers
    └── provider coupling

After
crates/
├── app-runtime
│   ├── startup / shutdown
│   ├── background jobs
│   ├── notifications
│   └── capability wiring
├── server
│   ├── routes -> capability entrypoints
│   └── mcp -> capability entrypoints
├── repos
├── tasks
├── execution
├── config
├── events
└── executors
    └── registry / facade only

Foundations kept shared:
- db / db-migration
- executors-protocol / executors-core / executor-*
- logs-protocol / logs-store / logs-axum
- utils-*
```

### 2) Introduce `app-runtime` as the only runtime composition root

A new crate, `crates/app-runtime`, will own:
- runtime startup and shutdown
- staged initialization
- background task registration
- domain service construction and wiring
- transport-facing state assembly for `server`

`deployment` and `local-deployment` will be removed rather than preserved as compatibility facades.

`server` will depend on `app-runtime` and domain-facing interfaces/facades. It will no longer receive a broad deployment object with getters for every subsystem.

**Alternatives considered**
- **Keep `deployment` but slim it down**: rejected because the existing name and shape encourage service-locator growth.
- **Let `server` own all composition directly**: rejected because it would keep runtime wiring and transport concerns coupled.

### 3) Split `services` by domain ownership, not by technical helper type

`services` will be deleted and its modules moved into capability crates with the following ownership map:

- `repos`
  - `project`
  - `repo`
  - `git`
  - `worktree_manager`
  - `filesystem`
  - `filesystem_watcher`
  - `file_search_cache`
  - `file_ranker`
  - `workspace_manager`
- `tasks`
  - task/attempt coordination
  - approvals lease/waiting coordination
  - archived kanban workflows
  - idempotency orchestration for task/start flows
- `execution`
  - container runtime services
  - execution process orchestration
  - queued message delivery
  - execution log backfill / runtime stream ownership
  - PR/runtime monitors tied to active executions
- `config`
  - config loading/saving
  - config version migrations
  - executor profile/runtime selection assembly
- `events`
  - event persistence, activity feeds, and domain event publication

This mapping favors stable business ownership over trying to separate every technical concern into micro-crates.

The concrete source-to-target mapping for the current repository is:

| Current area | Target crate | Notes |
| --- | --- | --- |
| `services/project.rs` | `repos` | Project CRUD and project-level repo coordination stay with repository ownership. |
| `services/repo.rs` | `repos` | Repository registration, repo metadata, and repo-scoped orchestration stay together. |
| `services/git/mod.rs`, `services/git/cli.rs` | `repos` | Blocking git execution stays inside repo-owned async-safe APIs. |
| `services/worktree_manager.rs` | `repos` | Worktree lifecycle belongs with repo topology and branch/worktree invariants. |
| `services/filesystem.rs`, `services/filesystem_watcher.rs` | `repos` | Workspace scanning and path access remain repo/workspace concerns. |
| `services/file_search_cache.rs`, `services/file_ranker.rs` | `repos` | Search index and ranking stay adjacent to repository discovery and filesystem scanning. |
| `services/workspace_manager.rs` | `repos` | Workspace root management remains colocated with repo discovery. |
| `services/approvals.rs`, `services/approvals/executor_approvals.rs` | `tasks` | Approvals are part of task/attempt control flow rather than provider/runtime infrastructure. |
| task/attempt orchestration currently spread across `server/routes/tasks*`, `server/routes/archived_kanbans.rs`, `server/routes/idempotency.rs` | `tasks` | Domain crate owns orchestration APIs; transport adapters keep request parsing only. |
| `services/container/mod.rs` | `execution` | Execution-process lifecycle, container ownership, and runtime status stay together. |
| `services/diff_stream.rs` | `execution` | Diff streaming is runtime-facing execution behavior built on repo APIs. |
| `services/queued_message.rs` | `execution` | Execution session queueing is runtime-owned. |
| `services/image.rs` | `execution` | Attempt/runtime image lifecycle stays with execution/session state. |
| `services/github.rs`, `services/github/cli.rs`, `services/pr_monitor.rs` | `execution` | PR/runtime monitoring is tied to active execution flows rather than base repo ownership. |
| `services/config/*` | `config` | Config schema, IO, and version migrations stay together. |
| `services/cache_budget.rs` | `config` | Runtime budget values are configuration policy; `app-runtime` reads and injects them into domains. |
| `services/events.rs`, `services/events/*` | `events` | Activity/event publication and stream shaping get a dedicated event domain. |
| `deployment/src/lib.rs` | `app-runtime` | Trait-based composition façade is removed; lifecycle assembly moves here. |
| `local-deployment/src/lib.rs` | `app-runtime` | Runtime construction, shutdown token wiring, and startup tasks move here. |
| `services/notification.rs` | `app-runtime` | OS notification delivery is a runtime side effect and should subscribe to domain outcomes rather than live inside a business domain. |
| `local-deployment/src/container.rs`, `command.rs`, `copy.rs` | `execution` | Local execution runtime helpers remain execution-owned even after composition moves out. |

The target route/MCP ownership after migration is:

| Transport area | Primary domain owner |
| --- | --- |
| `routes/projects.rs`, `routes/repo.rs`, `routes/filesystem.rs` | `repos` |
| `routes/tasks.rs`, `routes/task_groups.rs`, `routes/task_deletion.rs`, `routes/archived_kanbans.rs`, `routes/approvals.rs`, `routes/idempotency.rs` | `tasks` |
| `routes/containers.rs`, `routes/execution_processes.rs`, `routes/images.rs`, `routes/sessions/*`, `routes/task_attempts/*` | `execution` |
| `routes/config.rs` | `config` |
| `routes/events.rs` | `events` |

`server` remains the HTTP/MCP adapter, but each route group and MCP tool module is expected to be wired through the domain shown above rather than through a broad shared state object.

### 4) Formalize executor layering without renaming the registry crate

The executor stack will become:
- `executors-protocol`: shared stable protocol and serialized types
- `executors-core`: shared runtime logic, normalization, command/env helpers
- `executor-*`: provider implementations
- `executors`: registry/facade only

Keeping the crate name `executors` avoids unnecessary rename churn while still changing its responsibility. The crate will no longer be a mixed home for provider implementations and shared runtime helpers.

Provider crates MUST depend only on `executors-core`, `executors-protocol`, `logs-*`, and low-level utilities. They MUST NOT depend on `server`, `app-runtime`, or capability crates.

### 5) Make blocking infrastructure boundaries explicit

Blocking `git2`, filesystem traversal, and worktree operations will be owned only by `repos` and `execution`. These crates will expose async-safe APIs and keep `spawn_blocking` / blocking worker details internal.

No other crate may call raw blocking git/filesystem primitives directly. This prevents future async regressions from reappearing in `server`, route handlers, or unrelated domains.

### 6) Preserve behavior while changing imports aggressively

This refactor is internally breaking but externally stable:
- HTTP routes, MCP tools, and wire schemas stay the same.
- Old crate/module entrypoints are removed rather than kept as long-lived compatibility shims.
- Rust call sites are updated in one pass to the new crate paths and interfaces.

This follows the repository rule to upgrade old internal patterns directly rather than carrying compatibility layers.

### 7) Config versioning remains explicit

This change does not intentionally alter persisted config semantics. If runtime composition changes force any config shape updates (for example, renamed runtime identifiers or startup-mode settings), the change MUST add a new config version and migration under `crates/services/src/services/config/versions` before that module is moved into `config`.

If no config shape changes are required, the migration section of the implementation SHOULD explicitly state that config versions are unchanged.

### 8) Use injected cross-domain contracts instead of ad hoc direct imports

Some domains naturally build on others, but the dependency direction must stay explicit:

- `execution` MAY depend on `repos` for repo/worktree operations needed by active runs.
- `tasks` MAY depend on `execution` to start/stop attempts and observe execution state.
- `events` MUST remain consumable by other domains without becoming a new service-locator crate.
- `config` SHOULD remain leaf-like for config loading/versioning; `app-runtime` injects its outputs into other domains instead of every domain depending on `config` directly.

This keeps the graph understandable for parallel work and prevents the new capability crates from degenerating into a different shape of catch-all runtime.

## Boundary Verification

The implementation MUST update `scripts/check-crate-boundaries.sh` to validate the target crate graph explicitly. The minimum required checks are:

### Dependency graph checks

- Protocol crates:
  - `cargo tree -p executors-protocol | rg '\b(axum|rmcp|executors-core|executor-[^ ]+) v'` returns no matches
  - `cargo tree -p logs-protocol | rg '\b(axum|rmcp|executors-core|executor-[^ ]+) v'` returns no matches
- Core/foundation crates:
  - `cargo tree -p db | rg '\b(axum|rmcp) v'` returns no matches
  - `cargo tree -p logs-store | rg '\b(axum|rmcp) v'` returns no matches
  - `cargo tree -p repos | rg '\b(axum|rmcp) v'` returns no matches
  - `cargo tree -p tasks | rg '\b(axum|rmcp) v'` returns no matches
  - `cargo tree -p execution | rg '\b(axum|rmcp) v'` returns no matches
  - `cargo tree -p config | rg '\b(axum|rmcp) v'` returns no matches
  - `cargo tree -p events | rg '\b(axum|rmcp) v'` returns no matches
- Persistence vs executor runtime:
  - `cargo tree -p db | rg '\b(executors|executors-core|executor-[^ ]+) v'` returns no matches
- Executor layering:
  - `cargo tree -p executors | rg '\b(server|app-runtime|repos|tasks|execution|config|events) v'` returns no matches
  - for each `executor-*` crate, `cargo tree -p <provider> | rg '\b(server|app-runtime|repos|tasks|execution|config|events) v'` returns no matches

### Source-level import checks

These checks complement `cargo tree` by catching direct code usage that may be hidden behind shared crates:

- `rg -n 'use (axum|rmcp)|axum::|rmcp::' crates/db crates/logs-store crates/repos crates/tasks crates/execution crates/config crates/events` returns no matches
- `rg -n 'use executor-|executor_[a-z]|executors_core::' crates/server/src` returns no matches for provider/core direct imports; `server` must go through `app-runtime`, focused crates, and protocol/adapter crates
- `rg -n 'git2::|Repository::open|WalkBuilder|notify::' crates/server crates/app-runtime crates/tasks crates/config crates/events` returns no matches; blocking repository/filesystem primitives stay inside `repos` or `execution`

### Ownership matrix enforced by the checks

```text
Allowed high-level direction
app-runtime -> repos, tasks, execution, config, events, executors
server -> app-runtime, repos, tasks, execution, config, events, logs-axum, *-protocol
execution -> repos
tasks -> execution

Forbidden high-level direction
db -> executors / executors-core / executor-*
repos|tasks|execution|config|events -> axum / rmcp
executors|executor-* -> server / app-runtime / repos / tasks / execution / config / events
protocol crates -> axum / rmcp / executor runtime crates
```

## Risks / Trade-offs

- **Large crate churn** → Mitigate by implementing in fixed batches with workspace checks after each batch.
- **Domain boundaries chosen incorrectly** → Mitigate by enforcing the dependency graph with CI before the migration is complete.
- **Runtime startup regressions** → Mitigate by moving composition into `app-runtime` before deleting `deployment` / `local-deployment`.
- **Executor breakage during provider split** → Mitigate by keeping `executors` as the stable registry/facade crate while provider internals move.
- **Config/version surprises** → Mitigate by auditing config shape changes explicitly and versioning only when semantics change.

## Implementation Sequencing Rules

The implementer MUST follow these sequencing rules to keep the branch continuously verifiable:

- Complete one numbered phase fully before starting the next phase.
- Prefer one phase per PR (or one stacked PR slice per phase) so every merge point is independently reviewable and reversible.
- Do not keep long-lived compatibility re-exports, duplicate facades, or dual wiring after a phase gate passes; update all internal call sites in the same phase.
- Do not delete an old crate until the replacement crate compiles, all inbound references are moved, and the phase acceptance gate passes.
- If a phase fails its acceptance gate, stop and fix that phase before proceeding.

Mandatory acceptance gate after every phase:

- `cargo check --workspace`
- `cargo test --workspace`
- `pnpm run backend:check`
- `./scripts/check-crate-boundaries.sh`

Additional runtime gate required after Phases 4-8:

- `cargo build -p server --bin mcp_task_server`
- Start the backend and smoke-test one core path relevant to the phase before moving on

## Migration Plan

### Phase 1 — Guardrails and crate scaffolding

Goal: create the target crate names and enforcement rails before moving logic.

Work:
- Add empty `repos`, `tasks`, `execution`, `config`, `events`, and `app-runtime` crates to the workspace.
- Update `scripts/check-crate-boundaries.sh` to target the new crate names and the target dependency matrix.
- Add source-level import checks so forbidden direct imports fail early.
- Keep behavior unchanged in this phase; no business logic moves yet.

Exit criteria:
- The workspace compiles with the new crate shells present.
- Boundary checks understand the target graph before any code migration starts.

### Phase 2 — Extract low-risk leaf capabilities: `config` and `events`

Goal: move the least coupled `services` areas first to reduce blast radius.

Work:
- Move `services/config/*` and `services/cache_budget.rs` into `config`.
- Move `services/events.rs` and `services/events/*` into `events`.
- Rewire direct consumers in `server`, startup code, and any helper modules to the new crates.
- Remove the old `services` entrypoints for these modules in the same phase.

Exit criteria:
- `config` and `events` compile as real crates with no fallback path through `services`.
- No route or runtime path still imports the moved modules from `services`.

### Phase 3 — Extract `repos`

Goal: isolate repository, git, filesystem, and workspace behavior behind one capability crate.

Work:
- Move `project`, `repo`, `git/*`, `worktree_manager`, `filesystem`, `filesystem_watcher`, `file_search_cache`, `file_ranker`, and `workspace_manager` into `repos`.
- Keep blocking `git2`, watcher, and filesystem traversal internals inside `repos`; expose async-safe APIs upward.
- Rewire `projects`, `repo`, and `filesystem` transport modules to use `repos`.
- Remove the old `services` entrypoints for these modules in the same phase.

Exit criteria:
- All repo/filesystem functionality resolves through `repos`.
- No crate outside `repos` and `execution` uses raw blocking repo/filesystem primitives directly.

### Phase 4 — Extract `execution`

Goal: isolate runtime/container behavior before touching higher-level task orchestration.

Work:
- Move `services/container/mod.rs`, `diff_stream.rs`, `queued_message.rs`, `image.rs`, `github.rs`, `github/cli.rs`, and `pr_monitor.rs` into `execution`.
- Move `local-deployment/src/container.rs`, `command.rs`, and `copy.rs` into `execution` in the same phase.
- Rewire execution-oriented routes and MCP helpers that only need runtime/container behavior.
- Keep execution's repo/worktree dependency flowing through `repos`, not through `server` or ad hoc imports.

Exit criteria:
- Execution/container runtime logic compiles from `execution` only.
- Server startup still works and execution-oriented smoke flows still run after the move.

### Phase 5 — Extract `tasks`

Goal: move task/attempt orchestration after `execution` is stable so task flows can depend on execution cleanly.

Work:
- Move `services/approvals.rs` and `services/approvals/executor_approvals.rs` into `tasks`.
- Move task/attempt orchestration, archived kanban flows, and idempotent create/start coordination into `tasks`.
- Rewire task-centric routes, approval flows, and MCP handlers to call `tasks`.
- Remove the old `services` entrypoints for these modules in the same phase.

Exit criteria:
- Task lifecycle logic depends on `execution` through explicit crate boundaries.
- Core flows such as create task, start attempt, and approval wait/resume still pass smoke tests.

### Phase 6 — Introduce `app-runtime` and retire `deployment` / `local-deployment`

Goal: move composition last, after the capability crates already exist and compile on their own.

Work:
- Move startup sequencing, shutdown wiring, notification delivery, background jobs, and runtime assembly into `app-runtime`.
- Rewire `server` startup to depend on `app-runtime` instead of a broad deployment façade.
- Remove `crates/deployment` and the remaining composition responsibilities from `crates/local-deployment` in the same phase.
- Keep `server` state narrow: capability entrypoints only, not a new service-locator object.

Exit criteria:
- The backend starts via `app-runtime`.
- `deployment` and composition-only `local-deployment` code are gone.

### Phase 7 — Slim `executors` to registry/facade only

Goal: reduce executor surface after the capability/runtime graph is stable.

Work:
- Reduce `executors` to registry/facade responsibilities.
- Move any remaining shared provider/runtime helpers into `executors-core`.
- Verify every `executor-*` crate remains independent of `server`, `app-runtime`, and all capability crates.

Exit criteria:
- `executors` is a thin registry crate.
- Provider crates depend only on `executors-core`, protocol crates, log foundations, and low-level utilities.

### Phase 8 — Final transport cleanup and legacy crate removal

Goal: remove the last broad transport-time coupling and finish the refactor without leaving transitional scaffolding.

Work:
- Rewire any remaining `server` route groups and MCP modules to capability-crate entrypoints.
- Split remaining oversized transport modules once ownership is stable.
- Delete `crates/services` after every moved area is already served by `repos`, `tasks`, `execution`, `config`, `events`, or `app-runtime`.
- Run the full workspace gate plus targeted core flow smoke tests.

Exit criteria:
- `services`, `deployment`, and `local-deployment` no longer sit on the main runtime path.
- The workspace passes the full acceptance gate with the final target graph.

Rollback strategy during implementation is phase-level revert of the current branch/PR slice; no later phase should be started until the prior phase is green and mergeable.

## Open Questions

- Whether `events` remains a standalone crate or is merged into `tasks` if the implementation shows it is too thin.
- Whether any small helper crates are needed between capability crates and `db` for shared query objects, or whether existing protocol crates are sufficient.
