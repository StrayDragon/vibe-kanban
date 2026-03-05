# Specs Compression Report (2026-03-05)

Generated for `openspec/specs/*/spec.md` after archiving completed changes.

## Inventory

- Specs: 37
- Requirements: 191 (unique titles: 191)
- Scenarios: 296 (unique titles: 295)

### Per-spec counts

| spec | requirements | scenarios |
|---|---:|---:|
| `workflow-orchestration` | 35 | 56 |
| `execution-logs` | 17 | 21 |
| `fake-agent-executor` | 9 | 10 |
| `mcp-task-tools` | 9 | 10 |
| `crate-boundaries` | 7 | 7 |
| `mcp-approvals` | 7 | 10 |
| `workspace-management` | 7 | 14 |
| `access-control-boundary` | 6 | 18 |
| `llman-profile-import` | 6 | 8 |
| `translate-conversation` | 6 | 9 |
| `install-app` | 5 | 8 |
| `mcp-activity-feed` | 5 | 9 |
| `persist-metadata` | 5 | 5 |
| `agent-image-input` | 4 | 4 |
| `config-management` | 4 | 7 |
| `connect-database` | 4 | 6 |
| `diff-preview-guardrails` | 4 | 5 |
| `editor-integration` | 4 | 6 |
| `agent-auto-retry` | 3 | 3 |
| `agent-version-selection` | 3 | 12 |
| `api-error-model` | 3 | 9 |
| `deployment-composition` | 3 | 3 |
| `dev-script-guardrails` | 3 | 4 |
| `execution-race-safety` | 3 | 3 |
| `filesystem-api-boundary` | 3 | 4 |
| `project-git-hooks` | 3 | 7 |
| `task-attempts` | 3 | 3 |
| `task-group-prompting` | 3 | 7 |
| `task-runtime-quality` | 3 | 3 |
| `cache-budgeting` | 2 | 3 |
| `file-search-index` | 2 | 2 |
| `refresh-task-attempts` | 2 | 4 |
| `static-asset-caching` | 2 | 2 |
| `test-stability` | 2 | 2 |
| `transactional-create-start` | 2 | 4 |
| `cli-dependency-preflight` | 1 | 3 |
| `tag-management` | 1 | 5 |

## Exact duplicates

### Requirement titles

- (none)

### Scenario titles

- 2x `Reject mixed pagination modes`

## High-similarity candidates (cross-spec)

- 0.917: `mcp-activity-feed` / Project activity can be tailed incrementally / **Reject mixed pagination modes**
        `mcp-task-tools` / Consistent pagination semantics / **Reject mixed pagination modes**
- 0.804: `api-error-model` / Attempt control and long-poll failures SHALL use stable error codes / **wait_ms misuse is structured**
        `mcp-activity-feed` / Attempt feed returns latest logs plus pending approvals / **wait_ms requires after_log_index**
- 0.678: `mcp-activity-feed` / Mixed pagination SHALL return a structured tool error / **Reject mixed pagination modes with structured error**
        `mcp-task-tools` / Consistent pagination semantics / **Reject mixed pagination modes**
- 0.625: `api-error-model` / Attempt control and long-poll failures SHALL use stable error codes / **Invalid control token is structured**
        `mcp-task-tools` / Mutating attempt tools SHALL require a valid control_token / **Follow-up requires control token**
- 0.584: `mcp-activity-feed` / Attempt feed returns latest logs plus pending approvals / **Attempt feed includes pending approvals**
        `mcp-approvals` / MCP can list approvals / **List pending approvals for an attempt**
- 0.572: `crate-boundaries` / TypeScript generation MUST use protocol-owned types / **Types generation stays protocol-first**
        `task-attempts` / DTOs remain discoverable and typed / **Type generation succeeds**

## High-similarity candidates (within a spec)

- 0.902: `editor-integration` / Open-editor endpoints reject requests when disabled / **Open task attempt in editor while disabled**
        `editor-integration` / Open-editor endpoints reject requests when disabled / **Open project in editor while disabled**
- 0.901: `workflow-orchestration` / Node interruption controls / **Stop node execution**
        `workflow-orchestration` / Node interruption controls / **Force stop node execution**
- 0.881: `access-control-boundary` / Token-protected API boundary / **Non-localhost requires token**
        `access-control-boundary` / Token-protected API boundary / **Non-localhost still requires token**
- 0.866: `project-git-hooks` / Project overrides global git hook skipping / **Project override disables hook skipping**
        `project-git-hooks` / Project overrides global git hook skipping / **Project override enables hook skipping**
- 0.752: `workflow-orchestration` / Task group deletion cascades tasks / **Delete TaskGroup cascades tasks**
        `workflow-orchestration` / Task group deletion cascades tasks / **Delete entry task cascades TaskGroup**

## Proposed merge actions (decision-complete)

These are the concrete edits to implement for compression while preserving normative behavior.

### Canonical sources

- `mcp-task-tools`: canonical for MCP pagination semantics and JSON structured outputs
- `api-error-model`: canonical for structured tool error shape + stable `code` list

### Actions

1) `mcp-activity-feed`
   - Remove duplicate scenario `Reject mixed pagination modes`; reference `mcp-task-tools` pagination requirement instead.
   - Remove duplicate requirement+scenario about mixed pagination structured error; reference `api-error-model` for structured tool error shape.
   - Remove duplicate requirement about `structuredContent`; reference `mcp-task-tools` structured output requirement.
   - Remove duplicate scenario `wait_ms requires after_log_index`; keep the behavior by referencing `api-error-model` (`code=wait_ms_requires_after_log_index`).

2) `mcp-task-tools`
   - Keep `Consistent pagination semantics` as canonical; make its error behavior reference `api-error-model` (avoid re-defining structured error shape in two places).
   - Keep tool annotations/outputSchema/structuredContent requirements as canonical sources.

3) `mcp-approvals`
   - Keep requirement title `Approvals tools SHALL return structuredContent` but rewrite as a delta: inherits `mcp-task-tools` structured output rules and adds approvals-specific required fields.

4) `api-error-model`
   - Ensure `mixed_pagination` appears in the stable `code` list (since it is relied on by multiple MCP specs).
