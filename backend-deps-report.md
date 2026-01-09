# Backend Dependency Report

Scope
- Source: Cargo.toml in workspace crates under crates/
- Method: ripgrep for crate identifiers in crate source; no build or tests executed.
- Note: false positives are possible for macro-only usage, feature-gated code, or build scripts.

Potential removals (no direct usage found)
- crates/deployment/Cargo.toml: serde_json (no refs in crates/deployment/src)
- crates/executors/Cargo.toml: directories, fork_stream (no refs in crates/executors/src)
- crates/local-deployment/Cargo.toml: bytes, json-patch, openssl-sys, reqwest (no refs in crates/local-deployment/src)
- crates/server/Cargo.toml: ignore, rand, secrecy, sha2 (no refs in crates/server/src)
- crates/services/Cargo.toml: base64, reqwest, secrecy (no refs in crates/services/src)
- crates/utils/Cargo.toml: reqwest, sqlx (no refs in crates/utils/src)

Upgrade candidates (manual review)
- Git dependencies pinned to a branch/commit:
  - ts-rs (git branch `use-ts-enum`)
  - codex-protocol, codex-app-server-protocol, codex-mcp-types (git rev)
  Consider bumping the commit or switching to crates.io releases if available.
- Security/compatibility updates to check (run `cargo outdated -w`):
  - tokio, axum, tower-http, reqwest, sqlx, tracing, git2, rust-embed,
    notify/notify-debouncer-full, json-patch, uuid, chrono
  Rationale: runtime/networking libs often ship patch fixes; evaluate in CI.

Follow-up checklist
- Validate unused deps with `cargo udeps` or `cargo machete` (if available).
- Remove confirmed unused deps and run `cargo check` / `cargo test --workspace`.
- For upgrades, bump one crate family at a time and run `pnpm run backend:check`
  plus `cargo test --workspace`.
