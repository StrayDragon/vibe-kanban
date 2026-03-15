## Context

Vibe Kanban relies on long-lived realtime connections (WS/SSE) for UI convergence:

- Entity-map streams (tasks/projects/execution processes/scratch/etc.) deliver JSON Patch via WebSocket.
- The global SSE `/api/events` delivers JSON Patch events used to invalidate React Query caches.

Current failure modes that surface as “triggered but no UI change”:

- Idle disconnects or transient network drops lead to missed patches.
- Clients cannot detect message gaps deterministically (no monotonic sequence).
- Invalidation currently depends on client-side parsing of JSON Pointer paths, which is brittle when backend patch shapes evolve.

There is already a partial server-side mitigation: broadcast lag can cause a “resync snapshot” to be emitted, but clients cannot reliably detect gaps or resume efficiently after reconnect.

## Goals / Non-Goals

**Goals:**

- Add gap-detectable sequencing (`seq`) to realtime messages without breaking existing clients.
- Support short-window resume (`after_seq`) for WS streams, falling back to explicit resync (snapshot) when resume is not possible.
- Provide backend-authored invalidation hints alongside patches/events, with client fallback to existing inference.
- Improve observability (explicit resync reasons) and reduce idle disconnect risk (heartbeats/keepalive).

**Non-Goals:**

- Durable replay across process restarts or unlimited history retention.
- Replacing or removing existing endpoints; this phase must be additive/backward compatible.
- Large frontend architectural refactors (Phase 2) or major dependency upgrades (Phase 4).

## Decisions

### 1) Additive message envelope (keep `JsonPatch` / `finished`)

**Decision:** Extend WS JSON messages by adding optional fields while preserving the existing discriminators:

- Patch: `{"seq": <u64>, "JsonPatch": [...], "invalidate": {...optional...}}`
- Finished: `{"seq": <u64>, "finished": true}`

**Rationale:** Existing clients that only check for `JsonPatch`/`finished` remain compatible. New clients can track `seq` and consume hints.

**Alternatives considered:**

- Introducing a new message `type` field and breaking old clients.
- Creating parallel v2 endpoints immediately: increases operational complexity.

### 2) Sequencing at the message store layer, not per-connection

**Decision:** Assign a global monotonic `seq` at the point messages enter the shared store, and provide both:

- a sequenced receiver for new endpoints (`(seq, msg)`), and
- the existing unsequenced receiver for backward compatibility.

**Rationale:** Per-connection sequencing cannot support resume. Sequencing at the store enables:

- history replay by `after_seq`
- consistent `seq` across connections
- bounded memory via existing history limits

### 3) Bounded replay (ring buffer semantics)

**Decision:** Support resume only within the store’s retained history window. If `after_seq` is older than the retained minimum, the server sends a full snapshot and the client treats it as resync.

**Rationale:** Keeps complexity and memory bounded while covering the dominant case (short disconnects, tab sleep/wake).

### 4) Backend-generated invalidation hints

**Decision:** Generate invalidation hints server-side based on the patch content and emit them alongside:

- WS JSON-Patch messages (as an optional `invalidate` field)
- SSE events (as a new event type, e.g., `invalidate`, keyed by the same `seq`)

**Rationale:** Removes brittle client dependence on patch path shapes. Allows backend to evolve patch paths without forcing frontend code changes.

**Fallback:** Clients continue to infer invalidations from JSON Pointer paths if hints are absent (older server).

## Risks / Trade-offs

- **[Store sequencing changes ripple across crates]** → Mitigation: keep existing unsequenced APIs; add new sequenced APIs side-by-side; migrate routes incrementally.
- **[Replay buffer too small]** → Mitigation: tune via env config; always fall back to snapshot resync when needed.
- **[Bad invalidation hints cause stale UI]** → Mitigation: emit hints as additive; keep client inference as fallback; add e2e that compares “hint path” vs “inference path”.
- **[Increased message size]** → Mitigation: keep hints small (IDs + flags only); avoid embedding full entities.

## Migration Plan

1) Backend: introduce store-level `seq` and a sequenced subscribe API while retaining existing unsequenced behavior.
2) Backend: update WS entity-map stream routes to accept `after_seq` and emit `seq` in message envelope (still containing `JsonPatch`/`finished`).
3) Frontend: update WS consumers to track `seq`, pass `after_seq` on reconnect, and trigger resync on detected gaps.
4) Backend: add invalidation hint generation + emission:
   - WS `invalidate` field
   - SSE `invalidate` event (SSE event `id` set to `seq`)
5) Frontend: consume backend invalidation hints when present; fallback to current patch parsing.
6) Add regression tests (backend unit tests + frontend e2e “disconnect/gap → recovery”).

Rollback strategy: disable resume/hints by feature flag or revert per-route changes; old clients continue to function.

Config versioning: no user config migrations are expected.

## Open Questions

- Do we need `seq` to be per-stream-type or global across all events? (Global is simpler for SSE; per-stream can reduce coupling.)
- Should servers emit an explicit `resync: true` marker on snapshot messages, or rely on the snapshot patch itself as the marker?
