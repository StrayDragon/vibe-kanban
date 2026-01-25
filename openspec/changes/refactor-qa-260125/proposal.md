# Change: QA 重构修复 260125

## Why
整合 QA 优先级问题修复（260118 + 260124）：安全、数据一致性、API 错误处理、前端加载、测试覆盖、模块化、任务组可提示性，以及代码韧性与重构。

## What Changes
- 添加可配置的访问控制边界，覆盖 HTTP、SSE 和 WebSocket。
- 规范 API 错误映射，统一为 4xx/5xx 状态码与 ApiResponse 错误负载。
- 任务与尝试的创建/启动流程事务化，并在失败时清理。
- 修复没有执行进程时的对话加载状态。
- 强化日志归一化韧性，避免 panic 并保持日志连续性。
- 稳定日志渲染的 key，减少日志视图中昂贵的重渲染比较。
- 保护 TaskGroup 工作流草稿状态不被服务端刷新覆盖。
- 为关键流程添加最小化路由级集成测试。
- 模块化 task_attempts 路由并完成格式化清理。
- Task Group 节点指令可编辑，并追加到提示词中。
- 统一错误处理，减少高风险路径中的 unwrap/expect。

## Impact
- Affected specs: access-control-boundary (new), api-error-model (new),
  transactional-create-start (new), task-group-prompting (new),
  execution-logs (add), workflow-orchestration (add).
- Affected code: server routes/middleware, services, DB models, frontend hooks/UI,
  config schema, shared types, executors.
- Compatibility: 错误状态码规范化可能影响假设错误仍返回 200 的客户端。
