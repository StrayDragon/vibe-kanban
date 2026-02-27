## 1. Tool UX + error guidance

- [ ] 1.1 Update MCP tool descriptions to a compact guidance template (Use when / Required / Optional / Next / Avoid) for the core workflow tools (verify: run `cargo test -p server mcp_task_server` and ensure tool descriptions render in an MCP client).
- [ ] 1.2 Add stable MCP error codes + actionable `hint` mappings for common recoverable cases (no session yet, blocked guardrails, ambiguous IDs, mixed pagination) (verify: unit tests assert `code/retryable/hint` fields in error JSON).

## 2. Schema tightening for higher hit-rate

- [ ] 2.1 Refactor `follow_up` request schema to enforce action-specific required fields (prompt required for send/queue; cancel has no prompt) and unambiguous targeting (verify: schema inspection test + runtime validation tests).
- [ ] 2.2 Extend `tail_attempt_logs` to support incremental tailing via `after_entry_index` and reject `cursor` + `after_entry_index` together (verify: add tests for both modes and for invalid mixed mode).
- [x] 2.3 Add optional `request_id` idempotency keys for mutating MCP tools (`create_task`, `start_task_attempt`, `follow_up` send/queue) and enforce conflict on key reuse with different payload (verify: repeated calls return same ids; conflicting reuse returns 409).
- [x] 2.4 Add retention/cleanup for stored idempotency keys (prune completed keys by TTL; treat stale in_progress keys as recoverable) (verify: `cargo test --workspace`; optionally set `VK_IDEMPOTENCY_*_TTL_SECS` low and observe pruning in logs).

## 3. Add missing MCP tools for “agent == UI operator”

- [ ] 3.1 Implement `list_executors` MCP tool returning executor ids, variants, `supports_mcp`, and `default_variant` (verify: call tool and start an attempt using a returned executor).
- [ ] 3.2 Implement `stop_attempt` MCP tool wrapping the attempt stop API with `force` support and clear errors when nothing is running (verify: spawn a running process in tests and stop it).
- [ ] 3.3 Implement `tail_session_messages` MCP tool to replay session transcript with `cursor/limit` paging and attempt→latest session resolution (verify: create a session with logs and fetch transcript pages).
- [ ] 3.4 Implement `get_attempt_file` MCP tool (and minimal API support if needed) for bounded file reads from the attempt workspace with `blocked/blocked_reason/truncated` (verify: size limit and path containment tests).
- [ ] 3.5 Implement `get_attempt_patch` MCP tool (and minimal API support if needed) for bounded patch retrieval for selected paths with guardrails and explicit blocking (verify: guard-triggered `blocked=true` and forced narrow-path success).

## 4. Docs + validation

- [ ] 4.1 Update `_MCP.md` to document the optimized tool set, default parameters, and the top 3 “Avoid” mistakes (verify: examples match actual tool schemas).
- [ ] 4.2 Add integration tests covering common agent mis-calls (missing prompt, dual IDs, mixed pagination) and validate returned `hint` suggests the right next tool (verify: `cargo test --workspace`).
