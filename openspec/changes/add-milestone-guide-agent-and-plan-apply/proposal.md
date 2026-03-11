## Why

Milestones are the right unit for goal-driven work, but today they are still expensive to set up and iterate on: operators must manually create tasks, wire topology edges, and keep objective/definition-of-done in sync with the graph. This friction makes milestones underused and increases the chance of incorrect dependency graphs (which then breaks sequential dispatch and topology-based branching).

We need a conversational planning workflow that turns a milestone objective into an executable milestone graph (nodes + edges) with a safe, auditable “preview then apply” step, so operators can iterate quickly without hand-editing topology.

## What Changes

- Add a **Milestone Guide Agent** surface inside the milestone workflow UI to plan the milestone via conversation (objective, DoD, node list, topology).
- Introduce a **versioned, machine-parseable Milestone Plan schema** that agents can emit and VK can validate.
- Add **Plan Preview + Apply** flows:
  - preview the plan (show the diff: new tasks/nodes/edges/metadata)
  - apply the plan atomically (create missing tasks, update milestone graph + metadata)
  - record planner provenance (who/what applied the plan)
- Add an optional deterministic **Auto-wire topology** helper (non-LLM) as a fallback for quick drafts.
- Make milestone-generated tasks visually distinct (badge / created-by marker) so operators can see which tasks came from the planner.

## Capabilities

### New Capabilities

- `milestone-planning`: A guide-agent assisted planning workflow that produces and applies a structured milestone plan (objective/DoD, nodes, edges, presets) with preview + explicit operator confirmation.

### Modified Capabilities

- `milestone-orchestration`: Extend milestone orchestration with “plan preview/apply” and planner provenance, without changing milestone dispatch semantics.

## Impact

- Backend: new milestone plan validation/apply endpoints; plan diff computation; transactional “apply” that can create tasks and update milestone graph safely; new audit/provenance fields.
- Frontend: milestone workflow page gains a planning tab/panel (chat + plan preview/diff + apply); new visual markers for planner-created tasks/nodes.
- Shared types: add TS/Rust types for the milestone plan schema and apply results.
- Tests: unit tests for plan validation/apply; e2e coverage for the full “chat -> preview -> apply -> graph updated” path.

## Goals

- Reduce milestone setup cost: no manual task pre-creation or manual edge wiring for common cases.
- Keep planning safe and auditable: plans are previewed, validated, and explicitly applied; no silent background mutations.
- Keep the system bounded: planning is iterative, but “apply” is a single, deterministic mutation step.

## Non-goals

- Fully autonomous “planner runs until done” loops (continuation is handled separately).
- Auto-merge to the project main branch by default.
- A new orchestration runtime separate from the existing milestone scheduler.

## Risks

- Plan schema drift can cause brittle parsing: mitigate with explicit versioning and strict validation.
- Over-automation could hide intent: mitigate with diff preview and operator-confirm apply.
- Partial apply can corrupt graphs: mitigate with one transactional backend apply endpoint.

## Verification

- `cargo test --workspace`
- `pnpm run check && pnpm run lint`
- `pnpm run e2e:test -- e2e/milestone-planning.spec.ts` (new)
- Manual smoke check:
  - create milestone
  - chat to produce a plan
  - preview diff and apply
  - confirm tasks/nodes/edges + milestone metadata updated correctly

