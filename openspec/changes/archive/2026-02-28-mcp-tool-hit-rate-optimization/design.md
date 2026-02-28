## Context

Vibe Kanban’s MCP server (`mcp_task_server`) is now used as an automation surface, not just a convenience layer for a human in the UI. In practice, LLM agents have predictable failure modes when calling tools:

- Ambiguous identifier targeting (`attempt_id` vs `session_id` vs `execution_process_id`)
- Cursor vs tail semantics (paging older history vs fetching new entries)
- “Optional” request fields that are required in specific actions (e.g. `prompt` for `send`)
- Confusing control verbs (`cancel` vs `stop` vs `kill`)
- Guardrail blocks (`blocked`) without an obvious recovery path, causing retry loops

These are not primarily “capability gaps”; they are **tool UX** problems: schema + descriptions do not sufficiently constrain or guide an agent into the correct call.

## Goals / Non-Goals

**Goals:**

- Increase tool-call hit rate by making the **correct call the easiest call** (clear tool names, strict schemas, short actionable descriptions).
- Encode the closed-loop workflow into the tool surface: status → logs → changes → artifacts → follow-up/stop.
- Keep responses token-efficient by default, with explicit opt-in for more detail.
- Provide consistent, actionable error payloads so agents can recover without guessing.

**Non-Goals:**

- Authentication/authorization changes (assume same-host deployment for now).
- Streaming over MCP (prefer pull-style pagination in MCP; SSE/WS remains in `/api`).
- Broad filesystem APIs; any artifact reading stays scoped to an attempt workspace with strict bounds.

## Decisions

### 1) Prefer single-purpose tools over “multi-action mega tools”

**Decision:** Keep tools narrowly scoped (e.g. `follow_up`, `stop_attempt`, `get_attempt_file`) rather than a single `attempt_control(action=...)`.

**Why:** LLMs perform better with tools that have a single primary intent. “Action enums” increase branch complexity and lead to missing required fields.

**Alternatives considered:**

- One mega-tool with `action`/`kind`: fewer tools, but much lower parameter accuracy and harder recovery logic.

### 2) Standardize tool descriptions with a short, fixed template

**Decision:** Every MCP tool description uses the same structure:

- `Use when:` (1 sentence)
- `Required:` (explicit field list)
- `Optional:` (explicit field list)
- `Next:` (recommended next tool call)
- `Avoid:` (1–2 common mistakes)

**Why:** Agents pattern-match. A stable format improves tool selection and reduces “guessing” about next steps.

**Trade-off:** Descriptions become slightly longer; we must keep each section compact.

### 3) Use branch schemas (`oneOf`) to enforce action-specific required fields

**Decision:** For tools with mode-dependent requirements (notably follow-up control), represent requests as tagged enums so the JSON Schema encodes:

- which fields are required for each mode
- which fields are mutually exclusive (e.g. `cursor` vs `after_entry_index`)

**Why:** The schema itself is the best prompt. Making “prompt is required for send/queue” machine-checkable improves hit rate dramatically.

**Alternatives considered:**

- Keep a single struct with many `Option<T>` fields + rely on runtime validation: simplest code, but worst agent accuracy.

### 4) Make ID targeting explicit and consistent across tools

**Decision:** When a tool can target multiple identifiers, the request MUST specify exactly one target type, and the server MUST return an actionable error if the resource is not yet available (e.g. “no session yet”).

**Why:** Agents frequently provide both IDs or guess the wrong one. Strictness prevents silent misrouting.

### 5) Normalize pagination semantics and add incremental tailing where appropriate

**Decision:**

- `cursor` always means “page older history”
- `after_*` always means “return only new items since X”
- The server rejects requests that provide both.

**Why:** This eliminates the most common log-tail bug: repeatedly reading the last N entries and deduping client-side by accident.

### 6) Standardize MCP error payloads with recovery hints

**Decision:** Tool errors return a consistent JSON payload (even when transported via MCP error channels) with:

- `code` (stable string)
- `retryable` (bool)
- `hint` (next best action, usually naming the next tool and required field)
- `details` (small structured data, optional)

**Why:** Without hints, agents retry the same call or branch incorrectly (e.g. keep calling `follow_up` when no session exists).

### 7) Transcript replay tool naming

**Decision:** Use `tail_session_messages`.

**Why:** This matches existing `tail_*` semantics (bounded, paginated, “latest-first”) and avoids agents assuming a full transcript dump.

### 8) Split artifact retrieval tools

**Decision:** Split artifact retrieval into `get_attempt_file` and `get_attempt_patch` (avoid a multi-mode `get_attempt_artifact(kind=...)`).

**Why:** Single-intent tools are easier for LLMs to select and fill correctly, improving first-pass hit rate.

### 9) request_id idempotency for mutating calls

**Decision:** Add optional `request_id` support for mutating MCP tools (notably `create_task`, `start_task_attempt`, and `follow_up` send/queue). The server MUST treat repeated calls with the same key and same payload as idempotent, and MUST reject key reuse with a different payload.

**Why:** External orchestrators and agents retry on timeouts. Without idempotency, retries create duplicate tasks/attempts/executions and degrade the board state.

### 10) Idempotency key retention / stale in_progress cleanup

**Decision:** Store idempotency records only long enough to support safe retries:

- Completed records: keep for 7 days by default (`VK_IDEMPOTENCY_COMPLETED_TTL_SECS`, set to `0` to disable pruning).
- In-progress records: treat as stale after 1 hour by default (`VK_IDEMPOTENCY_IN_PROGRESS_TTL_SECS`, set to `0` to disable stale cleanup).

The HTTP server prunes old records periodically, and stale `in_progress` keys are deleted so clients can recover instead of looping on 409s forever.

**Why:** Prevent unbounded DB growth and avoid permanent “in progress” conflicts if the server crashes mid-request.

## Risks / Trade-offs

- **Breaking changes**: stricter schemas or renamed/split tools can break existing orchestrators → mitigate via a short deprecation window when we do care about compatibility.
- **Token overhead**: richer descriptions and errors add tokens → mitigate with short templates and minimal defaults.
- **Maintenance burden**: more tools means more code paths → mitigate by sharing request/response helpers and keeping tool behavior thin wrappers over existing `/api` routes.

## Migration Plan

- Implement new/updated MCP tools and schema constraints first.
- Update `_MCP.md` and any orchestrator examples to follow the new guidance format.
- Add tests for the most common agent failure modes (missing required fields, wrong pagination parameter, wrong control verb).
- If compatibility matters later: keep old tool names as deprecated aliases for one release, returning warnings in `hint`.

## Open Questions

- None (retention is TTL-based pruning + stale `in_progress` cleanup via `VK_IDEMPOTENCY_*_TTL_SECS`).
