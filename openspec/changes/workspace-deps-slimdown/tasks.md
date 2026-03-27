## 1. 删除死代码 crate（utils-jwt）

- [ ] 1.1 确认 `crates/utils-jwt` 无引用后从 workspace members 移除并删除 crate（或保留但不作为 member，按团队习惯）
- [ ] 1.2 运行 `cargo test --workspace` 确认无残留引用

Verification:
- `cargo test --workspace`

## 2. 非核心 executors 移出默认编译面（保持 opt-in）

- [ ] 2.1 根 `Cargo.toml` 调整 workspace members：默认只包含核心 executors（Claude Code + Codex）相关实现
- [ ] 2.2 确保 `crates/executors` 的 feature-gating 仍可显式启用非核心 executors（路径依赖可用，但不进入默认 `--workspace`）
- [ ] 2.3 CI/脚本调整：core job 只验证默认 features；增加可选 job（手动/定期）验证非核心 executors 不腐烂（可选）

Verification:
- `cargo test --workspace`

## 3. TLS 收敛到 rustls（减少 openssl-sys）

- [ ] 3.1 为使用 `reqwest` 的 crate 显式切换到 `rustls-tls` 并关闭默认 TLS features
- [ ] 3.2 移除/减少 `openssl-sys` 直依赖（保留 git2 等必要链路后再评估完全移除）
- [ ] 3.3 增加基本回归测试或 smoke（涉及网络调用的单测/集成测试，确保行为不回归）

Verification:
- `cargo test --workspace`

## 4. 依赖与 crate hygiene

- [ ] 4.1 合并/移除 `utils-git`（仅保留必要函数，避免把 git2 依赖链引入 config 路径）
- [ ] 4.2 OS dirs 依赖统一：优先使用 `directories`，逐步替换 `dirs`/`xdg`
- [ ] 4.3 清理 manifest 冗余依赖（移除“只在 test/间接使用”的直依赖声明）

Verification:
- `cargo test --workspace`

## 5. rust-embed feature 化（支持 server-only）

- [ ] 5.1 `crates/server`：前端 embed 放入 `embed-frontend` feature（默认开启）
- [ ] 5.2 `crates/utils-assets`：音频 embed 放入 `embed-sounds` feature（默认开启）
- [ ] 5.3 server-only 形态行为：API/MCP 正常；UI 路由返回 404（不依赖 frontend/dist 存在）
- [ ] 5.4 文档更新：新增 server-only 构建/运行说明（集群场景）

Verification:
- `cargo test --workspace`

## 6. Git 依赖策略调研与决策（git2 vs git CLI）

- [ ] 6.1 盘点当前 git 能力点：哪些功能必须依赖 git2，哪些可用 git CLI 等价替换
- [ ] 6.2 比较两条路线：兼容性（不同 git 版本差异）；依赖/构建复杂度（libgit2/openssl）；行为一致性与可测试性
- [ ] 6.3 输出 decision 文档（例如 `docs/decisions/git-strategy.md`）：明确推荐路线与分阶段拆除计划（避免长期双实现）
