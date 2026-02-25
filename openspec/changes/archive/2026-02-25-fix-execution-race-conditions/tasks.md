## 1. Implementation
- [x] 1.1 Add a completion-finalization guard/state to prevent duplicate finalize/commit flows.
- [x] 1.2 Route manual-stop and natural-exit paths through one synchronized completion path.
- [x] 1.3 Move repo-count transition handling into a transactionally safe post-insert check.
- [x] 1.4 Add concurrency-focused tests for stop-vs-exit and simultaneous add-repo requests.

## 2. Verification
- [x] 2.1 `cargo test --workspace`
- [x] 2.2 `openspec validate fix-execution-race-conditions --strict`
