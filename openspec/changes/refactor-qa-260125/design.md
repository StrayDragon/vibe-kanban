## 背景
本变更整合两次 QA 审计的修复清单：
- 260118：P0-SEC-01, P1-DATA-01, P1-ERR-01, P1-FE-01, P1-TEST-01, P2-MOD-01, P2-FMT-01, P2-TG-01
- 260124：日志归一化韧性、工作流草稿保护、错误传播规范化、模块拆分

## 目标 / 非目标
- 目标：
  - 新增访问控制、提升数据一致性、规范 API 错误、修复前端加载、补充测试。
  - 拆分大型路由模块、启用任务组指令提示。
  - 将高风险路径的 panic/unwrap 替换为结构化错误与日志记录。
  - 在异常序列下仍持续完成执行日志归一化。
  - 在刷新时保留 TaskGroup 工作流草稿。
  - 通过稳定标识降低日志视图的重渲染成本。
- 非目标：
  - 用户账号体系、OAuth/RBAC、多租户数据模型、大规模 UI 重构、新的编排引擎。
  - 不在无兼容方案的情况下变更 API 形状。
  - 不直接编辑 `shared/types.ts`。

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
- 将韧性改进视为显式行为变化，并通过规范增量记录。
- 采用范围收敛的小步 PR，并在每步显式验证。
- 通过保留错误来源（`thiserror`/`anyhow::Context`）来统一错误传播，而非字符串化错误。

## 备选方案
- 单个大型重构 PR：由于评审风险与回滚成本过高而否决。
- 端到端重写日志管道：超出范围而否决。

## 风险 / 取舍
- token 配置错误可能导致客户端无法访问；默认保持 disabled。
- 错误状态码规范化可能影响依赖 200 + 错误负载的客户端。
- 替换 `unwrap/expect` 时可能改变错误消息或状态码。
  - 缓解：尽量保留原有错误外观并补充定向测试。
- 保留 TaskGroup 草稿时可能改变 UI 行为。
  - 缓解：仅在 dirty 状态下保持草稿，并按需更新 UI 测试。

## 迁移计划
- 仅在新增字段时提升配置版本；默认值保证旧配置兼容。
- 回滚方式为关闭 access control 模式。
- 预计不需要数据迁移。
- 若后续需要配置变更，使用 `crates/services/src/services/config/versions` 并提升版本号。

## 未决问题
- 日志归一化错误是否需要独立的条目类型，还是复用现有错误条目结构？
- 是否需要共享的类型化 patch/event builder，以消除 services 中的 JSON 转换？
