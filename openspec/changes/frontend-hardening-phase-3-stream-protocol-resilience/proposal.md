## Why

Realtime UI consistency currently depends on multiple long-lived streams (WS/SSE). When streams disconnect, drop messages, or deliver patches out of order, users can experience “triggered but no UI change” even though the backend state changed.

This phase defines a forward-compatible stream protocol that is gap-detectable, resumable, and provides backend-authored invalidation hints to reduce brittle client-side patch parsing.

## What Changes

- Add monotonic sequencing to realtime streams so clients can detect gaps and converge reliably:
  - WS JSON-Patch streams include a `seq` field per message.
  - Clients track last seen `seq` and can request resume (`after_seq`) on reconnect.
  - If resume is not possible, servers send a full snapshot and the client treats it as an explicit resync.
- Add backend-provided invalidation hints alongside patches/events so clients do not need to infer invalidations by parsing JSON Pointer paths.
- Add heartbeats/keepalive and clearer close/reconnect semantics to reduce idle-time disconnects and improve recovery.

## Capabilities

### New Capabilities

- `realtime-stream-resilience`: Sequenced, resumable, and self-healing realtime streams with explicit invalidation hints for consistent UI convergence.

### Modified Capabilities

- (none)

## Impact

- Backend:
  - Event stream producers (patch generation) and WS/SSE routes that deliver realtime updates.
  - Potential in-memory buffering to enable short-window resume.
- Frontend:
  - WS/SSE stream consumers (`useJsonPatchWsStream`, event invalidation) and reconnection/resync logic.
  - React Query invalidation paths (prefer backend hints; fallback to existing client inference).
- Types/testing:
  - Stream message shapes (additive fields) and new e2e tests to simulate disconnect/gap scenarios.

## Goals

- Stream consumers can detect and recover from gaps deterministically (resume when possible; otherwise resync via snapshot).
- UI converges to canonical backend state after transient disconnects without requiring manual refresh.
- Invalidation logic becomes less brittle by using backend-provided hints, while remaining backward compatible.

## Non-goals

- Full offline support or durable replay across server restarts.
- Broad architectural re-org of the frontend (Phase 2) or major dependency upgrades (Phase 4).
- Replacing existing endpoints with breaking protocol changes (this phase is additive/backward compatible).

## Risks

- Adding buffering for resume increases memory usage and complexity.
- Compatibility mistakes could break older clients if message shapes change incompatibly (must remain additive).
- Incorrect invalidation hints could cause stale UI if not validated against existing behavior.

## Verification

- Local “just run” validation:
  - `pnpm run e2e:just-run` with scenarios that simulate stream gaps/disconnects and assert UI recovery.
- Protocol compatibility:
  - Existing clients that only look for `JsonPatch`/`finished` fields continue to work when extra fields are added.
- Backend correctness:
  - Targeted Rust tests for sequencing/resume logic and hint generation.
