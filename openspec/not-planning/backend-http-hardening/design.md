## 背景

后端是一个 Axum server，为 React frontend 提供服务，并在 `/api/*` 下提供 JSON API。应用通过 WebSocket 与 SSE 端点（例如 `/api/events`）进行实时状态流式传输。目前 API responses 没有做压缩。

后端还会发起出站 HTTPS 请求（例如 translation routes、OAuth flows 以及其他 integrations）。当前 `reqwest` 至少在 `crates/server` 与 `crates/execution` 中被固定在 `0.12`。

## 目标 / 非目标

**目标：**
- 为 `/api/*` 增加透明的 gzip + brotli 响应压缩，不改变任何 response body 的 shape。
- 保证 streaming 端点（SSE）仍正常工作（无缓冲、客户端不被破坏）。
- 将 `reqwest` 升级到 `0.13`，并使用 OS certificate store verification，以提升企业环境下的兼容性。

**非目标：**
- 不改变 WebSocket/SSE 的 payload 协议格式。
- 不改动 authentication / access-control 行为。
- 不新增 remote/cloud deployment 相关组件。

## 决策

### 决策：对 `/api/*` 使用 `tower_http::compression::CompressionLayer`

启用 `tower-http` 的 brotli 与 gzip 压缩特性，并在 API router 边界增加 `CompressionLayer`（而不是对所有路由全局启用）。

关键行为：
- 客户端支持时优先使用 brotli（`br`），否则回退到 gzip。
- 对已编码（already encoded）的响应不再重复压缩。

### 决策：显式将 SSE（`text/event-stream`）排除在压缩之外

SSE 依赖增量 flush；压缩可能导致缓冲并破坏 realtime UX。我们将通过压缩 predicate（或仅对非 SSE router 分层）排除 `Content-Type: text/event-stream` 的响应压缩。

同时确保 WebSocket upgrade responses 不受影响（它们不携带普通 HTTP body）。

### 决策：升级到 `reqwest` 0.13，并使用 rustls + platform verifier

将 `reqwest` 升级到 `0.13`，并使用 `default-features = false` + `rustls` feature set，通过 rustls 的 platform verifier 咨询 OS certificate store（与 upstream 行为一致）。这可以避免在 OS trust store 存在企业根证书（corporate root CA）时出现 TLS failures。

保持变更最小：不重构所有 HTTP 调用点，仅在 API 有变更处做必要调整。

## 风险 / 取舍

- **[SSE 缓冲风险]** → 排除 `text/event-stream` 响应压缩，并增加 streaming smoke test。
- **[依赖波动]**（`reqwest` + rustls feature 变化）→ 只升级必要的 crates，运行 `cargo test --workspace`，并补充一个覆盖 HTTPS client 构造的 unit test。
- **[TLS stack 行为差异]** → 记录所选 feature flags，并避免混用多个 `reqwest` major versions。

## 迁移计划

1. 启用 `tower-http` compression features，并在 API router 增加压缩 layer，同时排除 SSE。
2. 将 `reqwest` 依赖升级到 `0.13`（并调整 feature flags）。
3. 运行 backend checks/lints/tests，并补充有针对性的 smoke tests。
4. 如出现问题，可通过回滚压缩 layer 和/或将 `reqwest` 重新固定回 `0.12` 进行回退。

## 开放问题

- 是否需要 env toggle 以便调试时关闭压缩？（默认：**否**，保持行为简单。）
- 是否也要对静态资源启用压缩？（默认：**否**，本次只关注 `/api/*`。）
