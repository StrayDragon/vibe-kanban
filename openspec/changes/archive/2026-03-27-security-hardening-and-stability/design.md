## Context

VK 在近期完成了以 OS config dir 为根的 file-first YAML 配置重构，并将 Settings UI 收敛为只读/引导型。该方向显著减少了“运行时写配置”的复杂度，但当前仍存在几类残留风险：

- 访问控制在配置错误时存在 fail-open（`mode=TOKEN` + 空 token 等于 disabled）。
- secret 展开（`{{secret.*}}`/`{{env.*}}`）会作用于 YAML 的所有字符串字段，若内部结构被 API 原样序列化（ExecutionProcess、project repo 脚本等），则可能把展开后的敏感值回传。
- 文件/资源边界：repo register/init 允许任意绝对路径；图片上传允许 SVG，并在同源 `/api/**` 以 `image/svg+xml` + `Cache-Control: public` 提供，存在 stored XSS/敏感内容缓存等风险。
- 本地脚本生成：部分 helper 会把 `program + args` 直接拼接成 bash 字符串执行，若参数可被配置覆盖，存在 shell injection 风险。
- reload/多文件加载与测试存在一致性与稳定性问题（混合快照/TOCTOU、sleep 断言、env 串扰）。

本设计以“删除能力面 + 明确 DTO 边界 + fail-closed”为核心原则：对外 API 只返回 public-safe 的视图，任何可能携带 secrets 的内部字段不得出现在响应中；访问控制配置错误不得导致放行。

## Goals / Non-Goals

**Goals:**
- Access control 在 `mode=TOKEN` 下确保 fail-closed：token 缺失/为空时拒绝 `/api/**`（HTTP/SSE/WS 一致），并给出可操作的诊断信息。
- 建立并强制使用 Public API DTO 边界：服务端路由不再直接序列化 DB model/运行时内部结构体；对外返回仅包含安全字段的 DTO。
- 对 config-derived 字段统一使用 public 视图（保留占位符或移除敏感字段），避免 `{{secret.*}}` 展开结果经由任何 API 回传。
- Repo register/init 路径必须受 workspace roots 约束（canonicalize + containment check）。
- 图片上传与服务安全化：禁止 SVG；修正缓存与响应头（禁止 `public` 缓存，添加 `nosniff`）。
- reload 串行化与提交原子化；降低多文件读取的 TOCTOU 风险；提升相关测试稳定性。

**Non-Goals:**
- 不调整 `justfile` 中 `just run` 的默认 `HOST=0.0.0.0`（本变更不触碰默认 host 行为，也不新增基于 host 的强制 token 规则）。
- 不恢复任何“远程触发本机副作用”的快捷打开/编辑类 API。
- 不引入新的图形化配置编辑器或运行时写 YAML/secret 的能力。

## Decisions

### Decision 1: access control 配置错误时 fail-closed

**Choice**: 当 `accessControl.mode=TOKEN` 且 `token` 缺失/为空时，`/api/**` 直接拒绝（返回 `500` 或 `401` 的标准 `ApiResponse` 错误包；推荐 `500` 表示服务端配置错误），同时记录 warn/error 日志与可观测状态。

**Why**: 当前“把它当 disabled 放行”属于高危 fail-open；该错误应尽早显式暴露并阻止数据面流出。

**Alternatives**:
- A) 继续当 disabled：不接受（高危）。
- B) 自动生成随机 token：会破坏可预测性且难以排障（不选）。
- C) 在 config loader 层直接判 invalid 并 fallback 默认 config：若默认是 disabled，仍会暴露（不选为唯一手段）；可作为辅助诊断，但路由层仍需 fail-closed。

### Decision 2: 用 DTO 边界替代“直接回传内部结构体”

**Choice**: 对外 API 返回显式 DTO（`ts-rs` 导出），由路由层完成映射与脱敏；禁止直接回传 `ExecutionProcess` 等 DB model。

**Why**: “不小心把敏感字段序列化出去”的风险在重构/字段演进中很难靠 code review 长期保证；DTO 是可持续的边界工具。

**Clarification**:
- `ExecutionProcess` 对外仅允许通过 `ExecutionProcessPublic` 暴露最小必要字段集合。
- 不提供“debug-only 本地开关”去放大对外字段集合；若未来确有排障需求，应通过新变更引入单独的、严格 token-gated 的调试端点（且默认关闭）。

**Alternatives**:
- A) 在全局 JSON 序列化层做通用字符串扫描/替换：实现复杂且易误伤；无法保证覆盖所有 secret 形态（不选）。
- B) 继续返回 model 但加 `#[serde(skip)]`：会影响内部使用与数据持久化语义，且容易在别处绕过（不选）。

### Decision 3: config 派生数据对外统一走 public 视图

**Choice**: AppRuntime 提供 `public_config`（保留占位符、移除敏感字段、或最小化呈现），路由层在涉及 config 的读取中优先使用 `public_config`；对必须返回的内容，改为“存在性/摘要”而非正文（例如 setup/cleanup 脚本）。

**Why**: secret 展开是执行侧需求，但不应影响对外可观测面；把“执行用配置”与“展示用配置”分离可以从源头阻断泄露。

**Alternatives**:
- A) 仅靠前端不展示：不可靠，API 仍可被直接调用（不选）。
- B) 在每个路由手工删字段：容易遗漏；仍建议，但应以 DTO 与 public_config 作为主机制。

### Decision 4: 图片安全策略采用“禁止 SVG + 私有缓存”

**Choice**:
- 上传侧拒绝 `.svg`（以及 mime `image/svg+xml`），避免 stored XSS 风险。
- `/api/images/*` 与 attempt image proxy 响应使用 `Cache-Control: private, max-age=...`（或 `no-store`，视使用场景取舍），并添加 `X-Content-Type-Options: nosniff`。

**Why**: SVG 在同源场景具备较高 XSS 风险面，且难以“完全安全地”在不牺牲体验的前提下支持；对任务截图等内容不应使用 shared/public cache。

**Alternatives**:
- A) 允许 SVG，但强制下载（Content-Disposition: attachment）：可选，但仍需额外处理 content-type/nosniff；优先简单禁用。
- B) 对 SVG 做 sanitizer：引入复杂依赖与绕过风险（不选）。

### Decision 5: repo register/init 复用 workspace roots 约束

**Choice**: 以现有 filesystem routes 的 `allowed_workspace_roots()` 语义为准，将 repo register/init 的 path 也纳入同样的 canonical containment check。

**Why**: 统一边界定义，避免同类漏洞在不同入口重复出现；并且和现有 “filesystem listing 仅限 workspace roots” 形成一致的安全模型。

### Decision 6: reload 原子切换采用“单快照提交”

**Choice**: reload 过程中构建一个临时快照（包含 runtime config、public_config、status、executor cache 等），最后一步以单写锁/一次 swap 的方式提交，保证读侧不会看到混合代；reload 触发需串行化（watcher + 手动 reload 不并发）。

**Why**: 当前分步写入/多结构体更新会导致混合快照；单快照模型是最直接的竞态消除方式。

**Alternatives**:
- A) 依赖多个锁的顺序约定：容易死锁或遗漏（不选）。
- B) 读侧加锁读全量：会扩大锁粒度并影响吞吐（不选为主方案）。

### Decision 7: 生成脚本/命令必须避免 shell injection

**Choice**: 当需要通过 bash 执行 “program + args” 时，必须对每个 argv 做安全的 shell quoting（或改为不经 shell 的结构化执行，如果 protocol/执行层支持）。

**Why**: 参数来源可能被配置覆盖或包含空格/特殊字符；直接 `args.join(" ")` 会形成注入面，且风险会在后续演进中被放大。

**Alternatives**:
- A) 依赖“参数永远可信/不含特殊字符”的约定：不可持续（不选）。
- B) 对整段脚本做黑名单过滤：容易绕过且误伤（不选）。

## Risks / Trade-offs

- [API Breaking] DTO 化与字段移除会影响前端与外部脚本 → 同步更新前端、`pnpm run generate-types`、并加回归测试覆盖常用路由。
- [可用性] TOKEN 模式 token 为空会导致所有 `/api/**` 不可用 → 在错误信息中明确提示如何修复（设置 token 或切换为 disabled），并在 `/api/config/status` 或 `/api/info` 的可观测字段中体现“当前是 misconfigured”。
- [性能] reload 快照化可能增加少量 clone/copy → 以结构化 snapshot（Arc/共享）降低复制成本；只在 reload 时发生，可接受。
- [功能回退] 禁止 SVG 可能影响少数用户 → 明确变更说明；如确有需要，后续可引入“仅下载”模式作为 opt-in。
- [兼容性] 对 helper 脚本生成进行严格 quoting 可能改变少数边缘参数的表现（例如依赖 shell 展开） → 以 argv 语义为准；必要时在文档中说明不再支持隐式 shell 展开。

## Migration Plan

- 无 DB migration。
- 发布步骤：
  1. 合入服务端 fail-closed + DTO 边界变更与前端适配。
  2. 合入图片上传限制与缓存头修正。
  3. 合入 repo register/init 路径约束。
  4. 合入 reload 原子化与 TOCTOU 缓解，补齐测试。
- 回滚策略：
  - 若 DTO 字段变更导致前端不兼容，可临时回滚前端适配或在服务端提供兼容字段（但尽量避免长期兼容层）。
  - 若 fail-closed 导致误锁（token 配置混乱），可通过编辑 YAML 修复；必要时临时切回 disabled（明确风险）。

## Follow-ups
DTO 最小字段集合以 `ExecutionProcessPublic` 等公共 DTO 为准；图片缓存头的具体参数作为后续性能调优项单独演进（安全默认优先，不阻塞本变更目标）。
