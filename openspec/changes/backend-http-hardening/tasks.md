## 1. API Response Compression

- [ ] 1.1 Enable `tower-http` compression feature flags (gzip + brotli)
- [ ] 1.2 Add `CompressionLayer` to the `/api/*` router boundary
- [ ] 1.3 Exclude SSE (`Content-Type: text/event-stream`) from compression
- [ ] 1.4 Add a small smoke test (or manual recipe) to verify `Content-Encoding`
      negotiation with `curl`

## 2. Reqwest 0.13 + OS Certificate Store

- [ ] 2.1 Upgrade `reqwest` to `0.13` in `crates/server` and `crates/execution`
- [ ] 2.2 Ensure feature flags use rustls + OS trust store verification (no
      bundled-roots-only behavior)
- [ ] 2.3 Run `cargo tree | rg reqwest` to confirm only one major version is in
      use

## 3. Verification

- [ ] 3.1 Run `pnpm run backend:check`
- [ ] 3.2 Run `pnpm run lint`
- [ ] 3.3 Run `cargo test --workspace`

