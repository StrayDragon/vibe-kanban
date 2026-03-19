## 1. API Response Compression

- [ ] 1.1 启用 `tower-http` compression feature flags（gzip + brotli）
- [ ] 1.2 在 `/api/*` router 边界添加 `CompressionLayer`
- [ ] 1.3 将 SSE（`Content-Type: text/event-stream`）排除在压缩之外
- [ ] 1.4 增加一个小的 smoke test（或手动步骤），用 `curl` 验证 `Content-Encoding` 协商

## 2. Reqwest 0.13 + OS Certificate Store

- [ ] 2.1 在 `crates/server` 与 `crates/execution` 中将 `reqwest` 升级到 `0.13`
- [ ] 2.2 确保 feature flags 使用 rustls + OS trust store verification（避免 bundled-roots-only 行为）
- [ ] 2.3 运行 `cargo tree | rg reqwest`，确认依赖图中只存在一个 `reqwest` major version

## 3. Verification

- [ ] 3.1 运行 `pnpm run backend:check`
- [ ] 3.2 运行 `pnpm run lint`
- [ ] 3.3 运行 `cargo test --workspace`
