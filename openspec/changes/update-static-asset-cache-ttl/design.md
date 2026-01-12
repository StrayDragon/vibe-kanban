## Context
Static assets are currently served with zero TTL, which prevents repeat-visit caching of hashed bundles.

## Goals / Non-Goals
- Goals: Long-lived caching for hashed assets; preserve shorter caching for non-hashed assets.
- Non-Goals: Changing asset hashing strategy or build outputs.

## Decisions
- Decision: Use filename hashing detection (e.g., `*-<hash>.<ext>`) to apply long-lived cache headers.
- Alternatives considered: Per-path allowlist; rejected due to maintenance overhead.

## Risks / Trade-offs
- Risk: Misclassification of non-hashed assets as hashed.
  Mitigation: Use conservative hash pattern (hex length threshold) and verify known paths.

## Migration Plan
- Deploy header change; verify DevTools cache insight and repeat-visit transfer savings.

## Open Questions
- Which server layer currently serves `/assets/*` and `/ide/*` in production?
