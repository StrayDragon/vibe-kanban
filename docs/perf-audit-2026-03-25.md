# 性能 & 体验优化探索报告（Baseline）— 2026-03-25

> 目标：为“彻底优化前后端性能 + UX，并尽可能极简化项目”建立可量化基线，梳理模块与关键链路，提出可删/可拆/可降依赖的候选项（不落地实现）。

## 0. 运行环境

- Repo：`/home/l8ng/Projects/__straydragon__/vibe-kanban`
- Node：`v22.14.0`
- pnpm：`10.28.1`
- Rust：`rustc 1.95.0-nightly (1ed488274 2026-02-25)`

---

## 1. 前端 Baseline（bundle 体积）

### 1.1 生产构建输出（Vite）

执行：

```bash
VITE_SOURCEMAP=true pnpm -C frontend run build
```

关键信号：

- `3797 modules transformed`
- `built in 1.33s`
- Vite 警告：有多个 chunk > 500kB（建议进一步 code-split）

### 1.2 产物总体体积（frontend/dist/assets）

（不含 `.map`）

- Total JS：`3.72 MB` raw，约 `1.15 MB` gzip
- Total CSS：`90.8 KB` raw，约 `16.6 KB` gzip

### 1.3 Top chunk（按 raw / gzip）

| Chunk | Raw | Gzip | 备注（从命名推断） |
| --- | ---: | ---: | --- |
| `TaskAttemptPanel-*.js` | ~1114 kB | ~348 kB | 任务尝试/日志/Follow-up/（含 diff 渲染等大依赖） |
| `index-*.js` | ~760 kB | ~218 kB | 主入口/基础 UI/全局依赖 |
| `json-editor-*.js` | ~432 kB | ~138 kB | 设置页 JSON schema editor（@rjsf/ajv 类） |
| `AgentSettings-*.js` | ~353 kB | ~104 kB | Agent 设置页 |
| `alert-*.js` | ~243 kB | ~80 kB | UI 组件聚合 chunk（Radix/Tailwind 等） |
| `MilestoneWorkflow-*.js` | ~223 kB | ~68 kB | 里程碑工作流（xyflow） |

### 1.4 初始加载的“主动 preload”

`frontend/dist/index.html` 会 preload：

- `button-*.js`（~166 kB raw）
- `alert-*.js`（~243 kB raw）
- `hooks-*.js` / `checkbox-*.js` / `dist-*.js` / `useMutation-*.js` / `types-*.js`

这会把“冷启动首屏”负载推高（即使路由页面懒加载做得不错）。

### 1.5 可疑的“生产常驻依赖”

`frontend/src/main.tsx` 中无条件渲染：

- `ClickToComponent`（来自 `click-to-react-component`，通常是 dev-only 工具）
- `VibeKanbanWebCompanion`（若不是必须常驻，建议按需加载/开关）

这两者大概率会直接抬升 `index-*.js` 体积与首屏初始化成本。

---

## 2. 后端 Baseline（编译耗时 & 二进制体积）

> 注意：仓库的 `.cargo/config.toml` 将默认 target-dir 指到 `/var/tmp/vibe-kanban/cache/cargo-target/`。为避免污染共享缓存，本次测量使用 `--target-dir /tmp/...` 做隔离。

### 2.1 dev 构建（cold-ish）timings

执行：

```bash
PERF_TARGET_DIR=/tmp/vk-perf-dev-$(date +%Y%m%d-%H%M%S)
mkdir -p "$PERF_TARGET_DIR"
cargo build -p server --timings --target-dir "$PERF_TARGET_DIR"
```

本次报告文件（可直接用浏览器打开）：

- `/tmp/vk-perf-target-20260325-103012/cargo-timings/cargo-timing-20260325T023012845Z-c06a8169d68cfe39.html`

结果摘要：

- Total time：`88.6s`
- Top 贡献（按单元耗时）：
  - `openssl-sys ... build-script (run)`：`40.9s`
  - `starlark`：`21.1s` / `20.2s`（出现两次单元）
  - `codex-app-server-protocol`：`13.2s` / `12.9s`
  - `sea-orm`：`13.1s`
  - `libsqlite3-sys ... build-script (run)`：`8.8s`

Workspace crates（`v0.0.143`）里相对更“重”的：

- `server`：`11.6s`
- `executors-core`：`5.9s`
- `db`：`5.6s`
- `executor-claude`：`4.8s`
- `execution`：`3.7s`

### 2.2 release 构建 timings

执行：

```bash
PERF_TARGET_DIR=/tmp/vk-perf-release-$(date +%Y%m%d-%H%M%S)
mkdir -p "$PERF_TARGET_DIR"
cargo build -p server --release --timings --target-dir "$PERF_TARGET_DIR"
```

本次报告文件：

- `/tmp/vk-perf-target-release-20260325-103939/cargo-timings/cargo-timing-20260325T023939978Z-c06a8169d68cfe39.html`

结果摘要：

- Total time：`196.4s (3m16.4s)`
- Top 贡献：
  - `openssl-sys ... build-script (run)`：`104.1s`
  - `libsqlite3-sys ... build-script (run)`：`90.2s`
  - `codex-app-server-protocol`：`71.2s`
  - `starlark`：`50.9s`
  - `sqlx-postgres`：`38.7s`（⚠️ 这说明 Postgres 相关特性被编进来了）
  - `image`：`36.6s`（连带 `moxcms` 等）

Workspace crates top（release 下更明显）：

- `executor-codex`：`46.0s`
- `executors-core`：`40.2s`
- `executor-claude`：`30.0s`
- `executor-fake-agent`：`29.6s`
- `server`：`29.2s`

### 2.3 release 二进制体积（本次隔离 target-dir）

目录：`/tmp/vk-perf-target-release-20260325-103939/release`

- `server`：`102M`
- `mcp_task_server`：`71M`
- `generate_types`：`10M`
- 该 target-dir 总计：`305M`

---

## 3. 核心链路与热点（按“用户感知/系统压力”）

### 3.1 前端热点

- `/tasks`：多 panel + 多 Provider（GitOperations / ClickedElements / Review / ExecutionProcesses 等）叠加，易引起重渲染与连接管理复杂度。
- Attempt 视图：`TaskAttemptPanel` 相关 chunk 明显最大，主要来自：
  - 日志虚拟列表（`react-virtuoso`）
  - 归一化对话渲染（大量卡片/图标/markdown/wysiwyg）
  - diff 渲染（`@git-diff-view/*`）
  - Follow-up 编辑器（`Lexical` WYSIWYG，依赖集合大）
- 实时连接：WebSocket（logs / diff / execution processes）+ SSE（events invalidation）

### 3.2 后端热点

- execution/container：
  - 进程生命周期、日志落库/回放、diff watcher（`notify_debouncer_full` + spawn_blocking diff 计算）
  - `VK_LOG_PERSISTENCE_MODE` 暗示存在 legacy（jsonl）与新表（log_entries）双路径债务
- repos：
  - `git2` + git CLI 混用（差异生成、冲突检测、worktree 管理）
  - 文件搜索缓存与 watcher（受 cache budget 配置影响）
- server routes：
  - logs streaming：WS 持续推送 JSON（序列化/反序列化成本、消息量大时的 backpressure/丢帧体验）
  - events：SSE invalidation（有 “lagged / resume_unavailable” 全量失效信号）

---

## 4. “极简化”候选清单（按收益/风险分层）

> 这里把“删/拆/换”拆成可决策的项。是否要做、做到什么程度，需要你们确认产品方向（比如是否必须支持所有 executor / 是否需要 Postgres / 是否要单文件分发）。

### A. 前端（首屏 & 任务尝试视图为核心）

1) **把 dev-only 工具从生产常驻链路移除**
- `ClickToComponent`：建议仅在 `import.meta.env.DEV` 下启用，或改为手动开关（避免进入生产 bundle）。

2) **Attempt 视图重度按需加载（把 1.1MB chunk 拆细）**
- 将 diff 渲染、WYSIWYG、JSON editor、复杂卡片渲染拆成 dynamic import（只在用户打开对应 panel 时加载）。
- 目标：把 `/tasks` 进入 attempt 的“首可用”做到轻量（先文本/简卡），高级渲染延迟加载。

3) **减少初始 preload**
- `index.html` 当前 preload `button/alert/...`，会把冷启动负载前移；可按实际 TTI/交互点再调整。

4) **Provider/连接生命周期收敛**
- 以 attemptId 作为关键边界，避免“未打开 attempt 也建立 WS 连接/订阅”。
- 目标：少连、少算、少 re-render。

### B. 后端（编译速度、体积、依赖复杂度）

1) **TLS 依赖策略：避免默认 vendored OpenSSL**
- dev timings：`openssl-sys build-script 40.9s`
- release timings：`openssl-sys build-script 104.1s`
- 候选方向：
  - 迁移到 `rustls`（尽量不经由 openssl）
  - 或至少让 vendored 变成 feature（默认使用系统 OpenSSL）

2) **数据库后端收敛：SQLite-only vs 多后端**
- release timings 里 `sqlx-postgres 38.7s` 很显眼。
- 若产品方向就是 SQLite-only：删掉 `sqlx-postgres` 特性（db + migration + 相关调用链）。
- 若必须支持 Postgres：强 feature gate，默认构建不带 Postgres。

3) **SQLite “bundled vs system” 策略**
- `libsqlite3-sys build-script 90.2s`（release）是巨头。
- 单文件分发需要 bundled；但若你们主要跑在有系统 sqlite 的环境，可做成 feature 可选。

4) **执行器（executor）拆分/插件化**
- 目前 server 会编进大量 executor（compile time + binary size 都被拉高）。
- 候选方向：
  - 以 feature gate 控制编译哪些 executor（最小化默认）
  - 或把 executor 做成独立进程/动态插件（server 只保留协议 + 调度）

5) **移除 legacy 日志持久化路径**
- `VK_LOG_PERSISTENCE_MODE` 暗示双写/双读兼容复杂度。
- 若你们接受 breaking change：可在一个版本窗口后彻底移除 legacy jsonl 路径，只保留 `execution_process_log_entries`。

6) **非关键能力 feature 化**
- `notify-rust`/`zbus`（桌面通知）
- `image`/`moxcms`（若只是截图/预览边缘能力）

7) **清理非 workspace 的遗留目录/产物**
- `crates/review/` 当前为空目录（不在 workspace members）
- `crates/services/bindings/`、`crates/utils/bindings/` 疑似旧时代的生成物（当前全局未发现引用）
- 若确认无引用：可直接删掉，降低仓库噪音与维护成本

---

## 5. 模块逐个“注意到”（workspace 级）

> 这里先给出“职责 + 性能/复杂度关注点 + 可能的极简化方向”。后续可按你们最痛的链路逐个 deep dive。

- `crates/server`：HTTP API + WS/SSE + rust-embed 前端；关注：WS JSON 编解码成本、静态资源策略、translation 依赖（reqwest/tls）
- `crates/app-runtime`：服务容器/部署抽象；关注：事件订阅策略（默认不回放历史是正确方向）、服务初始化成本
- `crates/execution`：容器/进程/日志/diff watcher；关注：diff 计算（blocking + watcher）、日志持久化双路径、并发与 backpressure
- `crates/repos`：git/worktree/文件搜索缓存；关注：git2 vs CLI 的取舍、缓存预算与 watcher 数、索引构建成本
- `crates/db` + `crates/db/migration`：SeaORM/迁移/模型；关注：是否需要 Postgres、日志表/索引策略、数据增长与归档策略
- `crates/events`：事件服务/invalidations；关注：history 保留、lagged 后的全量失效体验
- `crates/logs-store`：MsgStore/history；关注：内存占用上限、快照/回放效率
- `crates/logs-protocol` / `crates/logs-axum`：协议与适配层；关注：序列化格式与传输效率
- `crates/executors-protocol`：executor actions/profile/shared types；关注：协议稳定性与前后端类型生成成本
- `crates/executors-core`：日志归一化/通用 executor 基建；关注：体积/复杂度是否可再拆（减少 server 热路径依赖）
- `crates/executors`：executor 汇总；关注：默认编译面过大
- `crates/executor-*`（claude/codex/gemini/...）：各家适配；关注：是否全量需要、是否可外置成插件/子进程
- `crates/tasks`：任务/状态/编排相关；关注：状态机复杂度与事件一致性
- `crates/config`：配置/缓存预算；关注：默认预算合理性、env 变量散落是否需收敛
- `crates/utils-*`：通用工具；关注：工具箱膨胀（可按领域拆分或减少跨层耦合）

---

## 6. 下一步建议（需要你确认方向）

为了把“探索”快速收敛成可执行重构，我建议你先回答两个关键问题：

1) **产品定位**：你们是否坚持“单文件/零依赖部署”？（这会强烈影响 openssl/sqlite 是否必须 bundled）
2) **executor 范围**：默认安装是否必须支持所有 executor？还是只需要 1-2 个主力，其余按需启用？

一旦确认，我可以把上述候选项整理成一个或多个 OpenSpec change（proposal/design/tasks），然后再进入实现阶段。

---

## 7. 建议路线图（按“先快后难”）

### Phase 0：定边界（0.5 天）

- 结论前置：**先明确“分发/部署形态”与“默认 executor 范围”**，否则后面的删/拆会反复。
- 输出：1 页决策记录（写清楚必须 bundled vs 可依赖系统库；默认带哪些 executor）。

### Phase 1：前端 Quick Wins（1–2 天）

目标：立刻降低首屏/任务页的加载与初始化成本，让用户“更快可用”。

- Gate `ClickToComponent`：仅 dev 启用（目标：降低 `index-*.js`）
- 审核 `VibeKanbanWebCompanion`：默认是否必须常驻（可选：延迟到需要时再挂载）
- Attempt 视图拆包：
  - diff 渲染（`@git-diff-view/*`）只在打开 Diffs panel 时加载
  - WYSIWYG（Lexical）只在进入编辑态时加载（读态用轻量 markdown renderer）
- 复测：重新 `pnpm -C frontend build` 对比 chunk 体积（至少把 `TaskAttemptPanel-*.js` 拆到多个按需 chunk）

### Phase 2：后端 Quick Wins（1–3 天）

目标：显著缩短 build 时间、降低二进制体积、减少依赖复杂度（提升 Dev UX + 运维体验）。

- SQLite-only 决策后：
  - 若 SQLite-only：移除 `sqlx-postgres`（期望：release timings 去掉 ~40s 单元，同时减小体积）
- TLS 依赖策略：
  - 让 vendored OpenSSL 变成可选（dev/release timings 的最大头部：40.9s / 104.1s）
- 可选能力 feature 化：
  - `notify-rust`/`zbus`、`image`/`moxcms` 等非关键能力按 feature 控制
- 复测：
  - `cargo build -p server --timings --target-dir /tmp/...`（看总时长和 top 单元是否下降）
  - `ls -lh .../release/server`（看体积是否显著下降）

### Phase 3：结构性简化（中等投入，回报最大）

目标：从“全量编译一切”转为“核心最小化 + 插件/按需扩展”。

- Executors：feature gate 或插件化（server 默认只带 1–2 个 executor）
- 日志持久化：移除 legacy JSONL 双路径，仅保留 `execution_process_log_entries`
- Realtime：评估 WS/SSE 的协议与前端渲染策略（减少 JSON 编解码、降低消息风暴的 UI 卡顿）

### Phase 4：建立持续的性能护栏（并行长期）

- Frontend：bundle size budget（CI 上报/阈值）
- Backend：build timings / binary size budget（CI 阈值）
- E2E：Playwright trace / 导出关键用户路径性能基线（TTI、长列表滚动、diff 打开耗时等）
