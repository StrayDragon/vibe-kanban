## Context

当前 workspace 默认以“全量 members + 全量 features”的方式参与 `cargo test --workspace`，即使我们已经把 executors 的默认能力收敛到 Claude Code + Codex，仍会在编译与依赖图中长期携带非核心实现与系统依赖（OpenSSL、git2 等）。这与“最小 core”目标冲突，也会持续放大 CI 时间与回归面。

## Goals / Non-Goals

**Goals:**
- 删除死代码 crate（workspace 维度）。
- 非核心 executors 从默认编译面移出，作为 opt-in feature/path 依赖存在。
- TLS 依赖优先收敛到 rustls（先从 reqwest 入手），降低系统依赖。
- embed-frontend/embed-sounds feature 化，支持 server-only 部署形态。
- 对 Git 依赖策略做结论并形成后续落地任务（git2 vs git CLI）。

**Non-Goals:**
- 不改变默认发行形态的用户体验（默认仍能单二进制运行并服务 UI）。
- 不在本变更里重写所有 git 相关业务逻辑（只做策略决策与必要的边界准备）。

## Decisions

1. **死代码直接删除**
   - 选择：移除未被引用的 workspace member（例如 `crates/utils-jwt`）。
   - 原因：无价值、只增加维护面。

2. **非核心 executors 从 workspace 默认 members 移出**
   - 选择：根 `Cargo.toml` 的 workspace members 只保留核心路径；非核心 executors 保留为 path 依赖但不参与默认 `--workspace` 编译。
   - 原因：显著降低 CI/本地编译成本；与 `executor-minimal-defaults` spec 一致。
   - 备选：保留 members 但在 CI 里排除。可行但更容易漂移，优先彻底移出。

3. **TLS：优先把 reqwest 统一为 rustls**
   - 选择：对所有使用 reqwest 的 crate 显式启用 `rustls-tls` 并关闭默认 TLS features；逐步撤掉 `openssl-sys` 直依赖。
   - 原因：减少系统依赖与构建失败点，供应链更简单。

4. **embed feature 化**
   - 选择：将“嵌入前端 dist + 嵌入音频资源”放在 `embed-frontend`/`embed-sounds` features 下；默认 features 仍开启。\n     server-only 节点可用 `--no-default-features` 启动，仅提供 API/MCP。
   - 原因：支持多 server 节点部署（只本地一个前端），并降低 server-only 节点体积/编译时间。

5. **Git 依赖策略先调研再决策**
   - 选择：以任务形式调研 git2-only vs git CLI-only：\n     - 兼容性（不同 git 版本差异）\n     - 依赖/构建复杂度（libgit2/openssl）\n     - 行为一致性与可测试性\n   - 输出：明确 1 条路线，并制定后续拆除另一条实现的计划。

## Risks / Trade-offs

- [可选 executor 腐烂] 移出 workspace 默认编译后缺少日常覆盖。
  - 缓解：新增 opt-in CI job（按周或手动触发）验证非核心 executors。
- [发行形态复杂度] feature 组合增加，需要文档清晰。
  - 缓解：默认 features 保持“全功能”；server-only 作为高级选项。
- [TLS 差异] rustls 与 openssl 行为差异极小但可能存在边缘差异。
  - 缓解：先收敛 reqwest；保留必要依赖后再逐步撤。

## Migration Plan

1. 删除死代码 crate（更新 workspace members + 相关引用）。
2. 调整 workspace members 与 executors feature/path 关系；确保默认 `cargo test --workspace` 仍通过。
3. reqwest 切换 rustls；移除/减少 openssl-sys 直依赖；跑全量测试。
4. embed feature 化：默认仍 embed；server-only 提供新启动方式与文档。
5. Git 依赖调研出结论后开后续变更落地（拆双实现）。

## Open Questions

- server-only 形态下，HTTP UI 路由返回策略：404 vs 提示“本节点不提供 UI”（更友好）。
- git2 是否仍是不可避免依赖（例如某些功能强依赖 libgit2），这会影响“完全去 openssl”可达性。

