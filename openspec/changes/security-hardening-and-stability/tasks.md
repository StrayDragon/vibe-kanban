## 1. Access Control Fail-Closed

- [x] 1.1 在 `crates/server/src/http/auth.rs` 中将 `accessControl.mode=TOKEN` 且 token 缺失/为空的情况改为 **fail-closed**（拒绝 `/api/**`，返回标准 `ApiResponse` 错误包），并输出可操作的诊断日志
- [x] 1.2 为 1.1 增加覆盖 HTTP + SSE + WebSocket 的回归测试（token 缺失/为空时均应被拒绝；`/health` 不受影响）
- [x] 1.3 校验并更新 `openspec/specs/access-control-boundary/spec.md` 的同步（归档时合并本次 delta spec）

Verification:
- `cargo test -p server auth`
- `cargo test --workspace`

## 2. ExecutionProcess API DTO 与脱敏边界

- [x] 2.1 引入对外 DTO（例如 `ExecutionProcessPublic`），并将 `/api/execution-processes/**` 相关路由从“直接回传 DB model”改为“回传安全 DTO”（不包含脚本正文、Authorization/header/token 等敏感字段）
- [x] 2.2 更新前端 `frontend/src/api/*` 与相关 hooks/pages，使其适配新的 ExecutionProcess DTO（确保 attempt 页面/日志页面不回归）
- [x] 2.3 运行并修复类型生成链路：更新 `crates/server/src/bin/generate_types.rs`（如需要），执行 `pnpm run generate-types`，确保 `pnpm -C frontend run check` 通过

Verification:
- `pnpm run generate-types`
- `pnpm -C frontend run check`
- `cargo test -p server execution_processes`

## 3. Project Repo API 不回传脚本正文

- [x] 3.1 调整 `GET /api/projects/{project_id}/repositories/{repo_id}`：不回传 `setup_script/cleanup_script` 等脚本正文，改为（可选）返回存在性字段（如 `has_setup_script`）或完全移除
- [x] 3.2 确保此类 config-derived API 使用 `public_config`（或等价的 redacted 视图），避免把 `{{secret.*}}` 展开结果回传
- [x] 3.3 更新前端相关页面/类型，使项目 repo 详情页不依赖脚本文本直出（改为引导查看 YAML）

Verification:
- `pnpm -C frontend run check`
- `cargo test -p server projects`

## 4. Repo Register/Init 的 Workspace Roots 约束

- [x] 4.1 为 `POST /api/repos` 与 `POST /api/repos/init` 增加 canonicalize + containment check，仅允许在允许的 workspace roots 下注册/初始化 repo；越界返回 `403`
- [x] 4.2 为 4.1 增加回归测试（inside roots 成功、outside roots 失败、symlink/traversal 失败）

Verification:
- `cargo test -p server repo`

## 5. 图片上传/服务安全化（禁 SVG + 私有缓存）

- [ ] 5.1 在 `crates/execution/src/image.rs` 中禁止 SVG（上传 `.svg`/`image/svg+xml` 返回 4xx），并为该行为增加测试
- [ ] 5.2 修正 `/api/images/{id}/file` 与 attempt image proxy 的响应头：移除 `Cache-Control: public`，改为 `private` 或 `no-store`，并添加 `X-Content-Type-Options: nosniff`
- [ ] 5.3 增加端到端回归测试：上传图片后可正常在 UI 渲染；SVG 上传被拒绝；图片响应头符合预期

Verification:
- `cargo test -p execution image`
- `cargo test -p server images`
- `pnpm -C frontend run check`

## 6. Shell 注入防护（helper scripts）

- [ ] 6.1 修复 `crates/server/src/routes/task_attempts/codex_setup.rs` 中 `program_path + args.join(\" \")` 的脚本拼接方式：对 argv 做安全 shell quoting（或改为结构化执行），避免 shell injection
- [ ] 6.2 为 6.1 增加回归测试：包含空格/引号/分号/换行等特殊字符的参数不会改变执行语义

Verification:
- `cargo test -p server codex_setup`

## 7. Reload 原子化与多文件一致性

- [ ] 7.1 将 reload 的“写入/提交”改为单快照原子切换（runtime config + public_config + status/diagnostics + executor cache 等不出现混合代）
- [ ] 7.2 watcher 自动 reload 与手动 reload 串行化（并发触发不竞态、不出现“旧覆盖新”）
- [ ] 7.3 降低多文件加载 TOCTOU 风险：对 `projects.d/*` 枚举/读取与 `secret.env`/`projects.yaml` 并行读取提供一致性策略（例如 generation/retry 或失败即保留 last-known-good）

Verification:
- `cargo test -p app-runtime reload`
- `cargo test --workspace`

## 8. 安全回归与测试稳定性清理

- [ ] 8.1 增加“secret 不外泄”回归测试：将 `{{secret.*}}` 注入到可配置字段中，验证相关 API 响应不包含展开后的 secret 值（ExecutionProcess、project repo 等重点覆盖）
- [ ] 8.2 清理剩余 flaky 测试：移除脆弱 sleep/墙钟阈值断言；确保 env 修改使用 RAII guard，避免 panic 后污染后续用例
- [ ] 8.3 文案/指引检查：确保用户提示统一指向 `projects.yaml` / `projects.d/*.yaml` 的最新配置路径（避免过时指向 `config.yaml` only）

Verification:
- `cargo test --workspace`
- `pnpm -C frontend run check`
