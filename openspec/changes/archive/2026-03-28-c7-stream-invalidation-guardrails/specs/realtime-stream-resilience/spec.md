# realtime-stream-resilience Specification (Delta)

## ADDED Requirements

### Requirement: SSE emits at most one event per sequenced patch update
The `/events` SSE stream SHALL emit at most one SSE event per sequenced JSON Patch update.

When the backend can derive invalidation hints for a sequenced JSON Patch update, the server MUST emit an `invalidate` SSE event with `id` equal to that update's `seq`, and MUST NOT also emit a `json_patch` SSE event with the same `id`.

When invalidation hints are not available for a sequenced JSON Patch update, the server MUST emit a `json_patch` SSE event with `id` equal to that update's `seq`.

#### Scenario: Hints available emits only invalidate
- **WHEN** a sequenced JSON Patch update has derivable invalidation hints
- **THEN** the server emits a single `invalidate` SSE event for that update's `seq`
- **AND** the server does not emit a `json_patch` SSE event with the same `id`

#### Scenario: Hints unavailable emits only json_patch
- **WHEN** a sequenced JSON Patch update does not have derivable invalidation hints
- **THEN** the server emits a single `json_patch` SSE event for that update's `seq`

### Requirement: SSE invalidate_all uses watermark identifiers
When the server cannot guarantee SSE stream continuity for a client, it SHALL emit an `invalidate_all` SSE event with `id` equal to the current stream watermark.

The `invalidate_all` payload MUST be valid JSON and MUST include:
- `reason`
- `watermark`

For `reason: "resume_unavailable"`, the payload MUST include:
- `requested_after_seq`
- `min_seq`
- `evicted`

For `reason: "lagged"`, the payload MUST include:
- `skipped`

#### Scenario: Resume unavailable emits invalidate_all with watermark id
- **WHEN** a client attempts to resume the SSE stream from an unavailable `after_seq`
- **THEN** the server emits an `invalidate_all` SSE event
- **AND** the event `id` equals the current `watermark`
- **AND** the payload includes `reason: "resume_unavailable"` and the required fields

#### Scenario: Lagged receiver emits invalidate_all with watermark id
- **WHEN** a client falls behind and the server drops SSE messages (lagged receiver)
- **THEN** the server emits an `invalidate_all` SSE event
- **AND** the event `id` equals the current `watermark`
- **AND** the payload includes `reason: "lagged"` and the required fields

