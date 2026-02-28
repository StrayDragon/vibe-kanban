## Context

Local deployment runs a periodic workspace cleanup loop. The existing behavior uses a fixed expiry cutoff and interval, which is inconvenient to tune across different operator preferences (disk pressure, many parallel projects, etc.). Environment variables are the lowest-friction configuration surface for local deployments.

## Goals / Non-Goals

**Goals:**
- Add environment-variable knobs to tune workspace expiry TTL and cleanup interval.
- Keep defaults unchanged.
- Document env knobs in a generated reference doc and enforce it in CI.

**Non-Goals:**
- Add new user-facing UI settings for these knobs.
- Add new heuristics for "smart" cleanup beyond existing running-process guards.
- Change cleanup behavior for non-local deployments.

## Decisions

- **Decision: Configure TTL/interval via env vars in local deployment.**
  - `VK_WORKSPACE_EXPIRED_TTL_SECS` controls the expiry cutoff computation.
  - `VK_WORKSPACE_CLEANUP_INTERVAL_SECS` controls the periodic tick interval.
  - `DISABLE_WORKSPACE_EXPIRED_CLEANUP` disables TTL-based cleanup while preserving the existing orphan cleanup control.
- **Decision: Clamp invalid or too-small values.**
  - Avoid accidental "delete everything immediately" misconfiguration by enforcing minimum bounds and logging warnings.
- **Decision: Generate `docs/env.gen.md` via a repo-local script.**
  - Provide a single source of truth for env knobs and a best-effort "Sources" section to reduce drift.
  - Add a CI check to ensure the generated doc is kept up to date.

## Risks / Trade-offs

- **Risk:** Smaller TTL values increase the chance of deleting uncommitted work.
  - **Mitigation:** Document the risk prominently in `docs/env.gen.md` and keep a conservative default (72h).
- **Trade-off:** Environment variables are less discoverable than UI settings.
  - **Mitigation:** Link the generated reference from `docs/operations.md`.

## Migration Plan

- No DB migrations required.
- Existing deployments keep the same behavior unless env vars are set.
- Operators can tune behavior by setting env vars and restarting the server.

