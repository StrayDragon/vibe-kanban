## 为什么

Vibe Kanban 在后端与 Web UI 之间经常传输体积较大的 API 响应（归一化日志、diff、任务列表等）。
为 `/api/*` 增加 HTTP 响应压缩可以减少带宽占用并改善体感延迟，尤其是在自托管且链路较慢/高延迟的场景中。

另一方面，一些处于 Cloudflare WARP 或企业代理后的用户会遇到 TLS 校验失败：网络环境依赖 OS 证书库中额外的信任锚点，
而应用若只依赖打包的根证书集则可能不完整。将 `reqwest` 升级到 `0.13` 并明确保证 HTTPS 校验会咨询 OS 证书库，
可以提升企业环境下的兼容性。

## 变更内容

- 为 `/api/*` 的 HTTP 响应启用 gzip + brotli 压缩，并对 streaming 端点（如 SSE）做显式排除。
- 将后端出站 HTTP 客户端升级到 `reqwest` `0.13`，并确保 HTTPS 证书验证**咨询 OS 证书库**（实现上可优先选择与 OS 证书库一致的 TLS 后端；如后续验证需要，再切换到更匹配的 verifier 方案）。
- 为两部分变更补充有针对性的 smoke 验证步骤/命令。

## 能力

### 新增能力

- `api-response-compression`：基于 `Accept-Encoding` 协商为 `/api/*` 响应启用 gzip/brotli 压缩，同时保持 streaming 语义不被破坏。
- `reqwest-0-13-os-cert-store`：出站 HTTPS 请求使用咨询 OS 证书库的验证器，以支持企业代理环境。

### 变更的能力

<!-- 无 -->

## 影响范围

- 后端路由层（`crates/server`）的 middleware/layer 组合，以及相关依赖 feature（如 `tower-http`）。
- `crates/server`、`crates/execution` 以及其他依赖 `reqwest` 的 crate 的出站 HTTP 逻辑。
- Cargo 依赖图与 `Cargo.lock`。

## 目标 / 非目标

**目标：**
- 在不改变 API shape 的前提下，降低大 JSON 响应的体积。
- 提升在 WARP/企业代理环境下的出站 HTTPS 兼容性。

**非目标：**
- 不做远程/云端架构改造。
- 不做 API 分页或响应结构重构。

## 风险

- 压缩可能影响 streaming 端点（SSE）→ 显式排除 `text/event-stream` 并通过流式 smoke test 验证（无缓冲、不中断）。
- `reqwest` 升级可能带来 TLS/HTTP 行为变化 → 增加聚焦的 smoke test，并运行现有 backend checks/lints/tests。

## 验证方式

- 使用 `curl` 携带 `Accept-Encoding: br` / `gzip` 访问 JSON API，响应应包含期望的 `Content-Encoding`。
- SSE 端点 `/api/events` 仍可正常 streaming（不被压缩导致缓冲）。
- 运行 `pnpm run backend:check`、`pnpm run lint`、`cargo test --workspace`。
