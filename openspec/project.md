# Project: Vibe Kanban Health + Remediation Plan

## Purpose
Provide a unified UI for multi-project, multi-agent management, focused on:
- Kanban task creation and lifecycle
- Task chat and agent execution tracking
- Merge/rebase workflows across repos

## Tech Stack
- Rust: axum, tokio, SeaORM
- React + TypeScript: Vite, Tailwind, React Query, Zustand
- SSE + WebSocket for events and logs
- ts-rs for shared type generation

## Goals
1. Add an explicit access-control boundary suitable for LAN/public deployment.
2. Improve data consistency in multi-step create/start flows.
3. Normalize API error model and HTTP status usage.
4. Fix known frontend loading bug and keep tests green.
5. Add minimal route-level tests for critical flows.
6. Reduce maintenance cost via targeted modularization.
7. Keep formatting and lint clean.

## Non-Goals / Right Boundary
- No multi-tenant data model in this phase.
- No user accounts, OAuth, or RBAC.
- No major UI redesign or framework migration.
- No breaking API changes without a compatibility path.
- No large performance rewrite beyond targeted fixes.

## Evidence-Backed Issues
- P0-SEC-01: No auth boundary for LAN/public; HTTP/SSE/WS/MCP are open.
- P1-DATA-01: Task/attempt create-and-start flows lack transaction/rollback.
- P1-ERR-01: Mixed HTTP status codes and ApiResponse error handling.
- P1-FE-01: useConversationHistory loading does not clear when no processes.
- P1-TEST-01: Missing server route integration tests.
- P2-MOD-01: Oversized modules (task_attempts.rs, frontend/src/lib/api.ts).
- P2-FMT-01: cargo fmt --check fails.
- P2-TG-01: Task Group not promptable/automation-friendly.

## Constraints
- Shared types must be generated (do not edit shared/types.ts directly).
- Config migrations live under crates/services/src/services/config/versions (if needed).
- Default local UX should remain functional with minimal friction.
- Any access-control must be configurable and documented.

## Config Migration & Compatibility
- Bump CURRENT_CONFIG_VERSION when adding or changing config semantics.
- New config fields must use serde defaults and normalization to preserve old files.
- Breaking config changes require a migration path (versions module + tests).
- Never return secrets (tokens, PATs) in API responses; redact in UserSystemInfo.
- Avoid removing fields; deprecate first and keep backward parsing aliases.

## Milestones
- M1: Access-control boundary (HTTP + SSE/WS + MCP).
- M2: Transactional create/start flows with rollback.
- M3: Error model normalization.
- M4: Frontend loading bug fix.
- M5: Route-level tests.
- M6: Modularization + format cleanup.
- M7: Task Group promptability.

## Verification Baseline
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --all-features -- -D warnings
- cargo test --workspace
- pnpm -C frontend run check
- pnpm -C frontend run lint
- pnpm -C frontend run test

## Risks and Rollback
- Auth changes can lock out clients -> rollback via config toggle.
- Transactional changes can break flows -> rollback via feature flag or revert.
- Error model normalization can affect clients -> compatibility mapping where needed.
