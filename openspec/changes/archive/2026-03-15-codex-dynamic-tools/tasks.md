## 1. Phase 1 — Codex protocol compatibility gate

- [x] 1.1 Add a runtime Codex protocol fingerprint check (schema generation + hash) with caching keyed by resolved Codex command identity (verify: unit test covers cache hit/miss behavior)
- [x] 1.2 Add an embedded “expected protocol fingerprint” derived from VK’s pinned `codex-app-server-protocol` schema output (verify: build/test can compute the expected fingerprint deterministically)
- [x] 1.3 Expose Codex compatibility status + diagnostics via an API surface used by Settings (verify: backend check `pnpm run backend:check`)
- [x] 1.4 Update Agent Settings UI to display Codex compatibility status and remediation copy (verify: `pnpm run check`)
- [x] 1.5 Enforce “incompatible ⇒ disabled”: block Codex spawn requests and return a user-actionable error message (verify: integration test or e2e scenario attempting to start Codex while incompatible)
- [x] 1.6 Add regression tests for known drift failures (e.g., unknown thread item variants in `thread/start`) to ensure VK fails fast with compatibility messaging instead of mid-run crashes (verify: `cargo test --workspace`)

## 2. Phase 2 — Codex Dynamic Tools (VK-native tools)

- [x] 2.1 Define a VK Dynamic Tool registry (tool names, descriptions, strict JSON schemas, argument validation) (verify: unit tests validate schemas reject invalid inputs)
- [x] 2.2 Register Dynamic Tools on `thread/start` when enabled (verify: executor-codex test asserts `ThreadStartParams.dynamic_tools` is populated)
- [x] 2.3 Implement `item/tool/call` request handling and dispatch to the registry (verify: unit test for unknown tool → `success:false` with explanatory text)
- [x] 2.4 Implement read-only tools: `vk.get_attempt_status`, `vk.tail_attempt_logs`, `vk.get_attempt_changes` (verify: happy-path tests return text output and `success:true`)
- [x] 2.5 Add approval gating plumbing for any mutating tools (even if no mutating tools ship initially) (verify: unit test shows approval is requested before execution)
- [x] 2.6 Add log normalization / UI rendering for Dynamic Tool activity (tool name + args summary + outcome) (verify: e2e or log normalization test produces `ToolUse` entries)
- [x] 2.7 Add an end-to-end smoke test that exercises a Dynamic Tool call in a Codex-backed attempt (verify: `pnpm run e2e:just-run` or a focused e2e spec)
