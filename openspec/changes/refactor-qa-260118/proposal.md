# Change: QA 重构修复 260118

## Why
解决已识别的 QA 优先级问题：安全、数据一致性、API 错误处理、前端加载、测试覆盖、模块化，以及任务组可提示性。

## What Changes
- 添加可配置的访问控制边界，覆盖 HTTP、SSE 和 WebSocket。
- 规范 API 错误映射，统一为 4xx/5xx 状态码与 ApiResponse 错误负载。
- 任务与尝试的创建/启动流程事务化，并在失败时清理。
- 修复没有执行进程时的对话加载状态。
- 为关键流程添加最小化路由级集成测试。
- 模块化 task_attempts 路由并完成格式化清理。
- Task Group 节点指令可编辑，并追加到提示词中。

## Impact
- Affected specs: access-control-boundary (new), api-error-model (new),
  transactional-create-start (new), task-group-prompting (new), execution-logs (add).
- Affected code: server routes/middleware, services, DB models, frontend hooks/UI,
  config schema, shared types.
- Compatibility: 错误状态码规范化可能影响假设错误仍返回 200 的客户端。
