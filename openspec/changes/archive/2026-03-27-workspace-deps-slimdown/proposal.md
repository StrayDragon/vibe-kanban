## Why

为了长期保持“最小 + 强大核心”，我们需要持续降低 workspace 的编译面、依赖面与维护面：
- workspace 内存在未被引用的 crate（死代码）与仅供可选能力使用的实现，但默认 `--workspace` 构建会把它们全部纳入编译/测试路径。
- 依赖链中仍存在较重的系统依赖（例如 OpenSSL），会带来跨平台构建失败点与供应链复杂度。
- 前端资源与音频资源的 embed 属于“发行形态选择”，但目前较难以 feature 方式选择“server-only 节点”（集群场景：只需要 API server，不需要嵌入前端）。
- Git 能力当前存在 git2 与 git CLI 的双实现倾向，长期维护成本偏高，需要明确方向。

## What Changes

- 删除 workspace 中未被引用的死代码 crate（例如 `crates/utils-jwt`）。
- 将非默认 executors 从 workspace 默认编译面移出：\n  - 默认构建/CI 只覆盖核心 executors（Claude Code + Codex）\n  - 其它 executors 仍可通过 feature 显式启用（保持扩展性，但不拖累 core）
- 将 HTTP 客户端 TLS 统一到 rustls（例如 `reqwest` 走 `rustls-tls`），逐步撤掉非必要的 `openssl-sys` 依赖。
- 合并/移除 `utils-git` 等超小 crate（避免将 `git2` 依赖链引入 config/核心路径）。
- 将 `rust-embed`（embed-frontend/embed-sounds）做成可选 feature：\n  - 默认发行形态仍支持“单二进制带前端”\n  - 同时支持“server-only”形态，用于多 server 节点部署（只本地一个前端）
- 调研并决策 Git 依赖方向：git2-only vs git CLI-only（考虑不同版本差异与可维护性），并给出落地计划。

Goals:
- 默认 `cargo test --workspace` 编译面显著变小，CI 更快更稳定。
- 减少系统依赖与供应链复杂度（优先 rustls）。
- 支持 server-only 部署形态（前端 embed 可选），以适配集群/分层部署。
- 明确 Git 依赖策略，避免双实现长期维护。

Non-goals:
- 不改变核心功能行为（默认发行形态仍提供前端、核心 executors 仍可用）。
- 不在本变更中删除在线翻译/诊断/文件系统等产品能力面。

Risks:
- feature/成员调整可能导致可选 executors 在缺少 CI 覆盖下腐烂。
  - Mitigation: 为可选 executors 提供独立的 opt-in CI job 或定期构建检查。
- rustls/openssl 切换可能影响极少数 TLS 行为。
  - Mitigation: 渐进切换，先收敛 reqwest；保留必要的 git 依赖后再评估完全移除 openssl。
- embed feature 化可能改变安装/运行文档与默认行为。
  - Mitigation: 默认 features 保持现状；server-only 作为显式选择并完善文档。

Verification:
- `cargo test --workspace`
- `pnpm -C frontend run check`（若涉及 embed/front build 脚本）

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `executor-minimal-defaults`: 默认构建与验证路径只覆盖核心 executors；非核心 executors 明确为 opt-in。
- `install-app`: 支持 server-only 构建形态（无前端 embed），用于多节点部署；默认仍支持单二进制嵌入前端。

## Impact

- Rust workspace: 根 [Cargo.toml](/home/l8ng/Projects/__straydragon__/vibe-kanban/Cargo.toml)、各 crate `Cargo.toml`、features/成员
- 依赖：`reqwest`/TLS、`openssl-sys`、`git2`/git CLI
- 资源 embed：`crates/server` 前端 embed、`crates/utils-assets` 音频 embed
- CI/脚本：`cargo test --workspace` 覆盖面调整，可能需要新增 opt-in jobs

