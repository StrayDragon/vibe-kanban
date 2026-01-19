## 1. 访问控制边界
- [ ] 1.1 添加 AccessControlConfig 架构、默认值和配置版本更新
- [ ] 1.2 为 /api 路由实现 HTTP 鉴权中间件，并在配置返回中脱敏 token
- [ ] 1.3 SSE/WS 通过 query param 校验 token；前端流式请求携带 token
- [ ] 1.4 添加 /api/info 和 /api/events 的鉴权边界测试

## 2. API 错误模型
- [ ] 2.1 集中化 ApiError -> StatusCode 映射
- [ ] 2.2 更新路由，使用 ApiError 替代临时 StatusCode 返回
- [ ] 2.3 补充或更新错误状态映射测试

## 3. 事务化创建/启动
- [ ] 3.1 在 create_task_and_start 中将 DB 写入包在事务里
- [ ] 3.2 在 create_task_attempt 中将 DB 写入包在事务里
- [ ] 3.3 start_workspace 失败时清理 workspace/workspace_repo
- [ ] 3.4 添加回滚测试验证部分创建失败

## 4. 前端加载修复
- [ ] 4.1 修复 useConversationHistory 的 loading 发射顺序
- [ ] 4.2 更新或确认 UseConversationHistory 测试

## 5. 路由测试
- [ ] 5.1 为 /api/tasks create/get 添加集成测试
- [ ] 5.2 为 /api/task-attempts create 添加集成测试

## 6. 模块化 + 格式化
- [ ] 6.1 拆分 task_attempts 路由到子模块，路径保持不变
- [ ] 6.2 运行 cargo fmt --all，保持仅格式化改动

## 7. Task Group 可提示性
- [ ] 7.1 在图更新时持久化 TaskGroupNode.instructions
- [ ] 7.2 从节点启动时将指令追加到提示词
- [ ] 7.3 在 TaskGroupWorkflow UI 中添加指令编辑
- [ ] 7.4 如果 Rust 类型变更，重新生成 shared types

## 8. 验证
- [ ] 8.1 cargo test --workspace
- [ ] 8.2 pnpm -C frontend run test
- [ ] 8.3 pnpm -C frontend run check && pnpm -C frontend run lint

## 9. 验收脚本
### 9.1 访问控制边界（HTTP/SSE/WS）
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
- allowLocalhostBypass=true 时，localhost 可无 token 访问；非 localhost 仍需 token。
- WebSocket 可用 `npx wscat` 或 `websocat` 连接验证 token 缺失/错误返回 401。

### 9.2 API 错误模型
```bash
export BACKEND_PORT=3001

# 404 NotFound
curl -i "http://localhost:${BACKEND_PORT}/api/tasks/00000000-0000-0000-0000-000000000000"

# 400 BadRequest（非法 UUID）
curl -i "http://localhost:${BACKEND_PORT}/api/tasks/not-a-uuid"
```
补充测试验证：
- 使用单元测试断言 ApiError -> StatusCode 映射覆盖 400/401/403/404/409/500。

### 9.3 事务化创建/启动
```bash
cargo test -p server transactional_create_start_rolls_back
cargo test -p server start_failure_cleans_workspace_records
```

### 9.4 前端加载修复
```bash
pnpm -C frontend run test -- useConversationHistory
```

### 9.5 Task Group 可提示性
```bash
cargo test -p services task_group_instructions_append_to_prompt
```
手工验证：
- 在 Task Group 工作流中编辑节点 instructions。
- 从该节点启动任务尝试，确认提示词包含指令内容。

## 10. 场景到测试用例映射
### 10.1 access-control-boundary
- 默认访问控制 -> crates/server/tests/routes_auth.rs::default_access_control_allows
- 默认允许 localhost bypass -> crates/server/tests/routes_auth.rs::default_localhost_bypass
- token 模式（HTTP 401/200）-> crates/server/tests/routes_auth.rs::http_token_required
- token 不匹配 -> crates/server/tests/routes_auth.rs::http_token_mismatch
- SSE token 缺失/有效 -> crates/server/tests/routes_auth.rs::sse_token_required
- WS token 缺失/有效 -> crates/server/tests/routes_auth.rs::ws_token_required
- 客户端 token 注入 -> frontend/src/lib/api.test.ts::injects_authorization_header

### 10.2 api-error-model
- 400/401/403/404/409/500 映射 -> crates/server/src/error.rs 单元测试

### 10.3 transactional-create-start
- 创建失败回滚 -> crates/server/tests/transactional_create_start.rs::create_failure_rolls_back
- 启动失败清理 -> crates/server/tests/transactional_create_start.rs::start_failure_cleans

### 10.4 execution-logs（前端加载）
- 空进程清空加载 -> frontend/src/hooks/UseConversationHistory.test.tsx::clears_loading_without_processes
- 加载中不清空 -> frontend/src/hooks/UseConversationHistory.test.tsx::keeps_loading_while_loading

### 10.5 task-group-prompting
- 指令持久化 -> crates/db/src/models/task_group.rs 单元测试
- 指令追加 -> crates/services/src/services/container.rs 单元测试
- UI 编辑 -> frontend/src/pages/TaskGroupWorkflow.test.tsx::edits_node_instructions
