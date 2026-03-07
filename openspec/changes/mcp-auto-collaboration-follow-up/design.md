## Context

VK already stores the truth needed for collaboration: task state, attempt state, review status, follow-up actions, and execution summaries. The problem is not missing orchestration state; the problem is that MCP callers have to reconstruct it from several low-level reads.

This change therefore adds a thin collaboration layer on top of the shipped auto-orchestration model. It should stay human-first:
- humans continue to use task detail, review inbox, approvals, and take-over actions
- MCP clients gain concise machine-readable surfaces for those same states
- task and attempt records remain the source of truth

## Goals / Non-Goals

**Goals:**
- Make review-ready managed tasks legible to MCP callers through one focused read contract.
- Persist explicit reasons when control shifts between human and automation.
- Keep executor/profile selection for auto-managed work policy-bound and inspectable.
- Reuse existing task/feed surfaces and additive DTO changes where possible.

**Non-Goals:**
- Build a second orchestration engine or a second review workflow.
- Introduce a new durable handoff table unless performance later requires it.
- Add push-first transport or long-lived MCP subscriptions in this change.
- Let MCP callers silently bypass project execution policy.

## Decisions

### 1. Reuse current task and attempt records as source of truth

The collaboration layer is derived from existing task/attempt state rather than stored in a parallel MCP-owned model. New persisted fields are limited to structured control-transfer reason and executor-policy diagnostics that do not already exist elsewhere.

### 2. Persist structured control-transfer reasons

Task orchestration state should persist a small enum-like reason set that covers at least:
- `human_pause`
- `human_takeover`
- `human_resume`
- `awaiting_human_review`
- `policy_rejected_profile`

These reasons must be readable from task detail, task list data, and MCP responses without log inspection.

### 3. Add project-level executor/profile policy for auto-managed work

Auto-managed MCP requests need an explicit project policy layer so callers can request automation without escalating rights implicitly.

Recommended shape:
- policy mode: `inherit_all` or `allow_list`
- allow-list entries as structured executor/profile identifiers
- persisted rejection diagnostic when a request is denied by policy

If these settings live in versioned project config, add a new latest config version with default `inherit_all` to preserve current behavior.

### 4. Provide a dedicated MCP handoff reader

Add a focused read tool such as `get_review_handoff` keyed by `task_id` or `attempt_id`.

The payload should include:
- task identity and orchestration state
- latest summary text
- concise diff summary
- validation outcome summary
- pending approval state
- recommended next actions (`approve`, `rework`, `take_over`, `resume_auto`)

This keeps machine parsing stable and avoids turning `get_task` into an oversized, unstable blob.

### 5. Enrich existing task/feed tools additively

Existing reads still need lightweight collaboration fields:
- `get_task` / `list_tasks`: control-transfer reason and effective policy result
- `tail_attempt_feed`: structured orchestration transition objects
- `tail_session_messages`: unchanged, still the deeper prompt/summary source for clients that need it

All changes should stay additive so current clients continue to function.

### 6. Publish orchestration transitions through the current feed path

Do not add a separate event bus. Reuse the existing activity/outbox path for transition types such as:
- `attempt_claimed`
- `retry_scheduled`
- `retry_blocked`
- `review_ready`
- `human_takeover`
- `automation_resumed`
- `profile_policy_rejected`

This keeps MCP and HTTP consumers aligned on one event vocabulary.

### 7. Keep human UI changes mirror-only

Human-facing changes should stay limited to showing the same transfer/policy reasons in existing task detail and review surfaces. This proposal does not introduce a second managed-task inbox, a separate agent dashboard, or divergent review actions.

## Migration Plan

- Add any new orchestration enums/fields in Rust DTOs and regenerate TypeScript types.
- If project policy is stored in versioned config, add a new latest config version and migration with conservative defaults.
- Keep all new API and MCP fields additive/optional so older clients degrade gracefully.
- Do not backfill derived handoff payloads; compute them from existing records at read time first.

## Risks / Trade-offs

- Derived handoff payloads can drift if their inputs are not read consistently.
- Policy, automation mode, and transfer reason can become confusing if surfaced separately instead of as one coherent diagnostic.
- Event feeds can become noisy if every internal write becomes a visible transition event.
