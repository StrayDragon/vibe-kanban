## Why

在完成 file-first YAML 配置重构后，系统整体暴露面已经显著收敛（设置页不再写配置、部分“快捷打开/编辑器打开”等副作用入口已移除），但仍存在若干高风险的“信息泄露 + 边界不清”的残留问题：

- **access control 存在 fail-open**：当 `accessControl.mode=TOKEN` 但 token 为空时，中间件会把它当作 disabled 放行，从而把所有 `/api/**` 暴露出去。
- **secret 展开结果仍可能经由 API 回传**：部分 API 直接回传运行时结构（例如 ExecutionProcess、project repo 的 setup/cleanup script 等），这些字段可能包含 `{{secret.*}}`/`{{env.*}}` 展开后的敏感值或用户直接写入的 secrets。
- **文件/资源边界存在可利用面**：repo register/init 接口允许提交任意绝对路径；图片上传允许 SVG 且通过同源 `/api/**` 以 `image/svg+xml` + 长期 public cache 提供，存在 stored XSS/缓存投毒/敏感内容缓存等风险。
- **一致性与稳定性问题**：配置 reload 与多文件加载（`config.yaml`、`projects.yaml`、`projects.d/*`、`secret.env`）仍可能出现混合快照/TOCTOU；部分测试依赖 sleep/全局 env 修改，存在 flaky 风险。

需要在“不增加新的可远程触发本机副作用能力”的前提下，对访问控制、API 输出、文件边界与 reload 语义做一次系统性加固。

## What Changes

- **BREAKING**: access control 在 `mode=TOKEN` 且 token 缺失/为空时 **fail-closed**（拒绝 `/api/**`，返回标准 `ApiResponse` 错误包），并提供清晰诊断日志/状态提示；避免“配置成 TOKEN 但实际等于 disabled”。
- **BREAKING**: 引入/完善“Public API DTO 与脱敏边界”，禁止将包含潜在 secrets 的内部结构体原样回传：
  - ExecutionProcess 相关 API 不再直接回传包含 `executor_action` 等敏感字段的 DB model；对外仅提供必要的可观测字段与安全的摘要。
  - Project repo 相关 API 不再回传 `setup_script/cleanup_script` 等脚本正文；改为返回存在性/摘要或完全移除该字段（由 UI 引导用户查看 YAML 文件）。
- 强化 secret 展开与配置可观测性边界：API 侧统一使用“不可泄露 secret 的视图”（例如保留占位符、或仅返回无敏感字段的 `public_config`），并补齐回归测试。
- 收紧 repo register/init 的路径边界：仅允许在“允许的 workspace roots”下注册/初始化仓库（canonicalize + containment check），拒绝越界路径。
- 图片上传/服务加固：
  - 禁止 SVG 上传（或以安全的下载方式提供而非同源 inline 渲染）。
  - 修正 `/api/images/*` 与 attempt image proxy 的缓存与安全响应头（不使用 `Cache-Control: public`，增加 `X-Content-Type-Options: nosniff` 等）。
- 配置 reload 与多文件加载的一致性增强：reload 串行化、提交原子化（单次 reload 生成单一快照并一次性切换），并降低多文件读取的 TOCTOU 风险。
- 测试稳定性清理：减少基于 sleep/墙钟的断言，统一 env 修改的 RAII guard，补充并行测试下的隔离。

## Capabilities

### New Capabilities
<!-- 本次变更优先以“修改既有能力”为主，不引入新 capability。 -->

### Modified Capabilities
- `access-control-boundary`: TOKEN 模式在 token 缺失时 fail-closed；补齐对 SSE/WS 的一致授权与更清晰的错误语义。
- `yaml-user-config`: 明确区分运行时配置与 public 可观测配置；限制 secret 展开结果出现在 API 输出中；强化 reload 原子性与多文件一致性。
- `execution-logs`: 执行进程/日志相关 API 输出不得包含可泄露 secrets 的字段（例如脚本正文、header/token 等），并通过 DTO/脱敏策略固化边界。
- `static-project-config`: 项目/仓库配置相关 API 不回传 setup/cleanup 脚本正文；并更新对 projects.yaml/projects.d 的最新语义约束。
- `filesystem-api-boundary`: repo register/init 等入口必须受 workspace roots 约束，禁止任意绝对路径越界。
- `frontend-security-cleanup`: 前端上传与渲染链路对不安全格式（SVG）做约束，并确保 UI 不依赖敏感配置字段直出。
- `static-asset-caching`: `/api/**` 下的用户/任务相关资源（例如上传图片）不应以 `public` 长期缓存方式提供。

## Impact

- Backend:
  - `crates/server/src/http/auth.rs`（认证中间件）
  - `crates/server/src/routes/execution_processes.rs`、`crates/server/src/routes/projects.rs`、`crates/server/src/routes/repo.rs`、`crates/server/src/routes/images.rs`、`crates/server/src/routes/task_attempts/images.rs`
  - `crates/app-runtime`/`crates/config`（reload 与 public_config 边界、多文件加载一致性）
- Frontend:
  - TS types（由 `ts-rs` 生成的 DTO 变更）
  - 项目/执行进程/图片渲染相关页面对字段变更的适配
- Tests:
  - Rust workspace tests（并发 env、时间断言、reload/脱敏回归）

## Reviewer Guide

- 优先关注“**不泄露**”与“**fail-closed**”：任何包含 secrets 的值都不应出现在 API payload/logs/前端直出中；访问控制配置错误不得导致放行。
- 关注边界一致性：repo path containment、reload 原子切换、SSE/WS 与 HTTP 授权语义一致。

## Goals

- 确保 access control 的配置错误不会导致 `/api/**` 无意暴露（fail-closed）。
- 将“可泄露 secrets 的内部字段”从所有对外 API 响应中系统性移除，并用 DTO 边界固化。
- 收紧文件/资源路径边界与上传内容安全，降低 stored XSS 与越界访问风险。
- 提升 reload/多文件加载一致性与测试稳定性，避免回归与 flaky。

## Non-goals

- 不处理 `just run` 默认 `HOST=0.0.0.0` 的变更（保持现状；本变更不调整 justfile 默认 host，也不新增基于 host 的强制 token 规则）。
- 不恢复任何“快捷打开/快捷编辑”等可远程触发本机副作用的 API 能力。
- 不引入新的图形化配置编辑器（仍以 YAML + schema 为主，UI 仅做状态/引导）。

## Risks

- API 响应字段收敛（例如移除脚本正文、ExecutionProcess DTO 化）可能影响现有前端/脚本使用；需要同步更新前端与类型生成，并补齐回归测试。
- 更严格的 access control 可能让现有“配置成 TOKEN 但没填 token”的环境直接不可用；需要提供明确错误提示与文档指引。
- 对图片格式与缓存策略的调整可能影响少数工作流（例如曾依赖 SVG 上传）；需在变更说明中明确。

## Verification

- 单元/集成测试覆盖：
  - `accessControl.mode=TOKEN` 且 token 缺失时 `/api/**` 被拒绝（HTTP/SSE/WS）。
  - ExecutionProcess / project repo 等 API 不包含脚本正文与敏感字段（包含 `{{secret.*}}` 展开后的值不得回传）。
  - repo register/init 对越界路径返回 `403`。
  - 图片上传拒绝 SVG；图片服务响应头不使用 `Cache-Control: public` 且含 `nosniff`。
  - reload 并发触发时序列化且提交快照原子切换（不出现混合状态）。

