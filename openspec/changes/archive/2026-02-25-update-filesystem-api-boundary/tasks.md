## 1. Implementation
- [x] 1.1 Add canonical path resolver that enforces workspace-root containment.
- [x] 1.2 Apply boundary checks to directory listing and git repo discovery endpoints.
- [x] 1.3 Add API error mapping for out-of-bound requests (`403`).
- [x] 1.4 Add backend tests for allowed and denied path cases.

## 2. Verification
- [x] 2.1 `cargo test -p server filesystem_ -- --nocapture` and `cargo test -p services --test filesystem_repo_discovery -- --nocapture`
- [x] 2.2 `openspec validate update-filesystem-api-boundary --strict`
