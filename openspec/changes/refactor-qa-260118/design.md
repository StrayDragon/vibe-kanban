## 背景
本变更将 QA 修复清单（P0-SEC-01, P1-DATA-01, P1-ERR-01, P1-FE-01,
P1-TEST-01, P2-MOD-01, P2-FMT-01, P2-TG-01）整合为一个基于规范的提案。

## 目标 / 非目标
- 目标：新增访问控制、提升数据一致性、规范 API 错误、修复前端加载、补充测试、
  拆分大型路由模块、启用任务组指令提示。
- 非目标：用户账号体系、OAuth/RBAC、多租户数据模型、大规模 UI 重构、
  新的编排引擎。

## 决策
- 访问控制通过 accessControl { mode, token, allowLocalhostBypass } 配置，
  默认 disabled 且允许 localhost bypass。
- HTTP /api 路由在 token 模式下需要 token；/health 保持公开，静态资源保持公开。
- SSE 与 WebSocket 在无法携带 header 时通过 query 参数传递 token。
- UserSystemInfo 中需要对 accessControl.token 做脱敏处理。
- ApiError 映射集中化，统一使用 4xx/5xx 状态码与 ApiResponse 错误负载。
- 任务/尝试创建与启动流程的 DB 写入放入事务，start_workspace 失败时清理。
- Task Group 节点指令持久化，并在从节点启动尝试时追加到提示词。
- 前端从 localStorage (vk_api_token) 注入 token 到 fetch/EventSource/WebSocket。

## 风险 / 取舍
- token 配置错误可能导致客户端无法访问；默认保持 disabled。
- 错误状态码规范化可能影响依赖 200 + 错误负载的客户端。

## 迁移计划
- 仅在新增字段时提升配置版本；默认值保证旧配置兼容。
- 回滚方式为关闭 access control 模式。

## 未决问题
- 无。
