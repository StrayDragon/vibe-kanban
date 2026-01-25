## 0. 上下文与边界
- 范围：访问控制边界、API 错误模型、事务化创建/启动、前端加载修复、路由测试、task_attempts 模块化、Task Group 指令提示、日志归一化韧性、工作流草稿保护、错误传播规范化。
- 不在范围：用户账号体系、OAuth/RBAC、多租户数据模型、大规模 UI 重构、新编排引擎。
- 关键定义：
  - accessControl = { mode: disabled|token, token, allowLocalhostBypass(default true) }
  - 保护范围：仅 `/api`（含 `/api/events` 与所有 WS）；`/health` 与静态资源保持公开
  - token 位置：Authorization Bearer 或 X-API-Token；SSE/WS 用 `?token=`
  - 前端 token 存储：localStorage `vk_api_token`
- 可观察/可测试/自动化目标：
  - 鉴权失败、事务回滚/清理必须有可检索日志
  - 新增行为必须有自动化测试覆盖；手工脚本仅作补充

## 1. 访问控制边界（HTTP/SSE/WS）
**边界**
- 仅保护 `/api` 路由（含 `/api/events` 与 WS）；`/health` 与静态资源保持公开
- token 允许 Authorization Bearer 与 X-API-Token
- SSE/WS 在无法使用 header 时仅允许 query param

**现状**
- 未搜索到 accessControl 配置、鉴权中间件或前端 token 注入逻辑

**实现**
- [ ] 1.1 添加 AccessControlConfig 架构、默认值、配置版本更新
- [ ] 1.2 为 `/api` 实现 HTTP 鉴权中间件（支持 allowLocalhostBypass）
- [ ] 1.3 SSE/WS 使用 `?token=` 校验（缺失/无效返回 401 + ApiResponse）
- [ ] 1.4 UserSystemInfo/config 响应脱敏 accessControl.token
- [ ] 1.5 前端统一注入 token（fetch/EventSource/WebSocket）
- [ ] 1.6 记录 accessControl 模式切换与鉴权失败日志（含路径/来源/原因）

**可测试（自动化）**
- [ ] 1.7 `/api/info` 与 `/api/events` 鉴权测试（401/200/localhost bypass）
- [ ] 1.8 WS 鉴权测试（token 缺失/有效 → 401/101）
- [ ] 1.9 前端 token 注入测试（HTTP header + SSE/WS URL）

**验收标准**
- disabled 模式下 `/api` 与 SSE/WS 无 token 可访问
- token 模式下缺失/错误 token 返回 401 且为 ApiResponse 错误负载
- allowLocalhostBypass=true 时 localhost 无 token 可访问，非 localhost 仍需 token
- `/health` 永远公开
- `UserSystemInfo/config` 不回传 token 明文

## 2. API 错误模型
**边界**
- 只规范错误状态码与 ApiResponse 错误负载，不改变成功响应结构

**现状**
- ApiError -> StatusCode 映射存在，但部分路由仍返回 200 + ApiResponse::error

**实现**
- [ ] 2.1 集中化并校验 ApiError -> StatusCode 映射（400/401/403/404/409/500）
- [ ] 2.2 将路由内 `ApiResponse::error(...)` 改为 `Err(ApiError::...)`
- [ ] 2.3 评估并对齐 `error_with_data` 端点的状态码语义
- [ ] 2.4 关键 5xx 输出 `tracing::error`（便于排查）

**可测试（自动化）**
- [ ] 2.5 ApiError 状态码映射单元测试
- [ ] 2.6 路由错误码集成测试（至少覆盖 400/404/409/500）

**验收标准**
- 错误响应均为非 200 状态码
- 响应体为 ApiResponse 错误负载（message 或 error_data）
- 典型错误（非法 UUID/不存在资源/冲突）返回 400/404/409

## 3. 事务化创建/启动
**边界**
- 仅影响 create_task_and_start 与 create_task_attempt
- 不改变既有业务校验与 API 结构

**现状**
- DB 写入未使用事务；start_workspace 失败不清理 workspace/workspace_repo

**实现**
- [ ] 3.1 create_task_and_start：task/task_image/workspace/workspace_repo 写入同一事务
- [ ] 3.2 create_task_attempt：workspace/workspace_repo 写入同一事务
- [ ] 3.3 start_workspace 失败后清理 workspace/workspace_repo（task 保留）
- [ ] 3.4 抽出清理 helper，保证幂等
- [ ] 3.5 记录事务回滚与清理失败日志

**可测试（自动化）**
- [ ] 3.6 创建失败回滚测试（无残留 task/workspace/workspace_repo）
- [ ] 3.7 启动失败清理测试（workspace/workspace_repo 被移除）

**验收标准**
- 任一写入失败不会留下部分记录
- start_workspace 失败后不会留下 workspace/workspace_repo
- 回滚/清理过程可通过日志追踪

## 4. 前端加载修复（对话历史）
**边界**
- 仅处理无执行进程时的 loading 状态，不改变日志流/历史拉取逻辑

**现状**
- useConversationHistory 已包含"无进程清空 loading"逻辑

**实现**
- [ ] 4.1 确认 loading 发射顺序，必要时修正
- [ ] 4.2 补充测试：空进程清空加载 & 加载中不清空

**可测试（自动化）**
- [ ] 4.3 UseConversationHistory 测试覆盖两种状态

**验收标准**
- 执行进程列表为空且加载完成时 loading=false
- 执行进程仍在加载时 loading=true

## 5. 日志归一化韧性与 UI 稳定性
**边界**
- 后端：日志归一化路径不 panic，异常时发出错误条目
- 前端：日志渲染使用稳定标识符

**现状**
- executors 中存在 panic/unwrap 路径
- 前端日志渲染使用 JSON.stringify 或 index key

**实现**
- [ ] 5.1 将日志归一化路径（executors）中的 panic/unwrap 替换为防护更新和错误条目
- [ ] 5.2 将事件 patch/stream 构建中的 `expect/unwrap` 替换为可失败构建并记录日志
- [ ] 5.3 用稳定身份（entry index/patchKey）与记忆化比较替代虚拟化日志中的 JSON.stringify 等价判断
- [ ] 5.4 将原始日志渲染中的 index key 替换为稳定 id

**可测试（自动化）**
- [ ] 5.5 日志归一化韧性测试（异常序列不 panic）
- [ ] 5.6 前端日志渲染稳定性检查（pnpm run check + lint）

**验收标准**
- 工具结果异常时流发出错误条目并继续处理
- 前置加载更早历史时既有渲染条目保持身份且滚动位置稳定

## 6. 工作流草稿保护
**边界**
- 仅影响 TaskGroup 工作流视图的刷新行为

**现状**
- 服务端刷新可能覆盖用户未保存的草稿

**实现**
- [ ] 6.1 刷新时保留 TaskGroup 工作流草稿，仅在非 dirty 状态下同步
- [ ] 6.2 对齐面板视图状态与用户意图（避免 effect 覆盖）

**可测试（自动化）**
- [ ] 6.3 工作流草稿保留测试（如已有测试环境）

**验收标准**
- 用户有未保存编辑时收到更新数据，UI 保留草稿
- 保存或丢弃后呈现最新服务端状态

## 7. 路由测试（集成）
**边界**
- 仅覆盖 `/api/tasks` 与 `/api/task-attempts` 核心创建/获取路径

**现状**
- 未发现对应路由级集成测试

**实现**
- [ ] 7.1 `/api/tasks` create/get 集成测试
- [ ] 7.2 `/api/task-attempts` create 集成测试

**验收标准**
- 正常创建/获取返回 200 + ApiResponse::success
- 错误场景返回规范化错误码与 ApiResponse 错误负载

## 8. 模块化 + 格式化
**边界**
- 路由路径保持不变，仅做结构拆分与格式化

**实现**
- [ ] 8.1 拆分 task_attempts 路由到子模块
- [ ] 8.2 将传输层专属辅助函数（如 LogMsg 的 SSE/WS 映射）下沉到 server 层
- [ ] 8.3 `cargo fmt --all`（仅格式化改动）

## 9. Task Group 可提示性
**边界**
- 仅增加 TaskGroupNode.instructions 持久化与提示词追加
- 不改变其他提示词内容与 UI 结构

**现状**
- instructions 字段与 UI 编辑已存在；提示词追加逻辑缺失

**实现**
- [ ] 9.1 更新图时持久化 TaskGroupNode.instructions（空白视为 null）
- [ ] 9.2 从节点启动时将非空指令追加到提示词
- [ ] 9.3 追加行为加入 debug 日志（便于定位）

**可测试（自动化）**
- [ ] 9.4 指令持久化单元测试（db 模型）
- [ ] 9.5 指令追加单元测试（services/container）
- [ ] 9.6 UI 指令编辑测试（TaskGroupWorkflow）

**验收标准**
- 指令保存后读取一致；清空后保持为空
- 指令仅在非空时追加到初始提示词

## 10. 前端表单与状态
**边界**
- 修复表单默认值与状态来源问题

**实现**
- [ ] 10.1 异步数据加载完成且表单干净时，重置 TaskFormDialog 默认值
- [ ] 10.2 统一 follow-up 消息状态来源，避免发送陈旧内容

## 11. 前端类型安全与 API 面
**边界**
- 提升类型安全，按域拆分 API

**实现**
- [ ] 11.1 用 schema 校验 AgentSettings 的 profiles JSON，移除不安全的强转
- [ ] 11.2 按域拆分 `frontend/src/lib/api.ts`，共享请求辅助函数

## 12. 后端错误传播规范化
**边界**
- 统一错误处理，保留 source errors

**实现**
- [ ] 12.1 规范图片服务与工具模块的错误传播，保留 source errors

## 13. Shared Types
- [ ] 13.1 如 Rust 类型变更，执行 `pnpm run generate-types`

## 14. 自动化验证（CI/本地）
- [ ] 14.1 `cargo test --workspace`
- [ ] 14.2 `pnpm -C frontend run test`
- [ ] 14.3 `pnpm -C frontend run check`
- [ ] 14.4 `pnpm -C frontend run lint`
- [ ] 14.5 如需：`pnpm run generate-types`
- [ ] 14.6 确认 CI 覆盖以上命令（如无则更新 CI）

## 15. 手工验收脚本（补充）
### 15.1 访问控制边界（HTTP/SSE/WS）
```bash
export BACKEND_PORT=3001

# 获取当前配置
curl -s "http://localhost:${BACKEND_PORT}/api/info" > /tmp/vk-info.json

# 写入 token 模式配置（本地绕过关闭）
python - <<'PY'
import json
info = json.load(open('/tmp/vk-info.json'))
config = info['data']['config']
config['accessControl'] = {
  'mode': 'TOKEN',
  'token': 'test-token',
  'allowLocalhostBypass': False
}
json.dump(config, open('/tmp/vk-config.json', 'w'))
PY

curl -i -X PUT "http://localhost:${BACKEND_PORT}/api/config" \
  -H 'Content-Type: application/json' \
  --data @/tmp/vk-config.json

# /health 公开
curl -i "http://localhost:${BACKEND_PORT}/health"

# /api 无 token -> 401
curl -i "http://localhost:${BACKEND_PORT}/api/info"

# /api 带 token -> 200
curl -i -H "Authorization: Bearer test-token" \
  "http://localhost:${BACKEND_PORT}/api/info"

# SSE 无 token -> 401
curl -i "http://localhost:${BACKEND_PORT}/api/events"

# SSE 带 token -> 200
curl -i "http://localhost:${BACKEND_PORT}/api/events?token=test-token"
```
补充手工验证：
- allowLocalhostBypass=true 时，localhost 可无 token 访问；非 localhost 仍需 token
- WebSocket 可用 `npx wscat` 或 `websocat` 连接验证 token 缺失/错误返回 401

### 15.2 API 错误模型
```bash
export BACKEND_PORT=3001

# 404 NotFound
curl -i "http://localhost:${BACKEND_PORT}/api/tasks/00000000-0000-0000-0000-000000000000"

# 400 BadRequest（非法 UUID）
curl -i "http://localhost:${BACKEND_PORT}/api/tasks/not-a-uuid"
```

### 15.3 事务化创建/启动
```bash
cargo test -p server transactional_create_start_rolls_back
cargo test -p server start_failure_cleans_workspace_records
```

### 15.4 前端加载修复
```bash
pnpm -C frontend run test -- useConversationHistory
```

### 15.5 Task Group 可提示性
```bash
cargo test -p services task_group_instructions_append_to_prompt
```
手工验证：
- 在 Task Group 工作流中编辑节点 instructions
- 从该节点启动任务尝试，确认提示词包含指令内容

## 16. 场景到测试用例映射（目标）
### 16.1 access-control-boundary
- 默认访问控制 -> crates/server/tests/routes_auth.rs::default_access_control_allows
- 默认允许 localhost bypass -> crates/server/tests/routes_auth.rs::default_localhost_bypass
- token 模式（HTTP 401/200）-> crates/server/tests/routes_auth.rs::http_token_required
- token 不匹配 -> crates/server/tests/routes_auth.rs::http_token_mismatch
- SSE token 缺失/有效 -> crates/server/tests/routes_auth.rs::sse_token_required
- WS token 缺失/有效 -> crates/server/tests/routes_auth.rs::ws_token_required
- 客户端 token 注入 -> frontend/src/lib/api.test.ts::injects_authorization_header
- SSE/WS URL 注入 -> frontend/src/contexts/EventStreamContext.test.tsx::adds_token_query

### 16.2 api-error-model
- 400/401/403/404/409/500 映射 -> crates/server/src/error.rs 单元测试
- 路由返回错误码 -> crates/server/tests/routes_errors.rs::maps_error_status_codes

### 16.3 transactional-create-start
- 创建失败回滚 -> crates/server/tests/transactional_create_start.rs::create_failure_rolls_back
- 启动失败清理 -> crates/server/tests/transactional_create_start.rs::start_failure_cleans

### 16.4 execution-logs
- 空进程清空加载 -> frontend/src/hooks/UseConversationHistory.test.tsx::clears_loading_without_processes
- 加载中不清空 -> frontend/src/hooks/UseConversationHistory.test.tsx::keeps_loading_while_loading
- 工具结果异常 -> crates/executors/tests/normalization_resilience.rs::tool_result_anomaly
- 前置加载更早历史 -> frontend/src/components/logs/LogView.test.tsx::prepend_preserves_identity

### 16.5 workflow-orchestration
- 编辑中服务端刷新 -> frontend/src/pages/TaskGroupWorkflow.test.tsx::preserves_draft_on_refresh

### 16.6 task-group-prompting
- 指令持久化 -> crates/db/src/models/task_group.rs 单元测试
- 指令追加 -> crates/services/src/services/container.rs 单元测试
- UI 编辑 -> frontend/src/pages/TaskGroupWorkflow.test.tsx::edits_node_instructions
