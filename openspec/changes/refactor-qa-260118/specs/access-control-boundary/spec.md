## ADDED Requirements
### Requirement: 访问控制配置
系统 SHALL 暴露 accessControl 配置，包含 disabled 或 token 的模式、共享
token 字符串，以及默认值为 true 的 allowLocalhostBypass 标志。

#### Scenario: 默认访问控制
- **WHEN** accessControl 未配置
- **THEN** 系统将访问控制视为 disabled，并允许无 token 请求

#### Scenario: 默认允许 localhost bypass
- **WHEN** accessControl.mode 为 token 且 allowLocalhostBypass 未设置
- **THEN** allowLocalhostBypass 视为 true

### Requirement: 禁用模式允许访问
当 access control 模式为 disabled 时，系统 SHALL 允许 HTTP、SSE 与
WebSocket 请求在无 token 的情况下访问。

#### Scenario: 禁用模式下的 HTTP 请求
- **WHEN** access control 模式为 disabled
- **THEN** /api 请求无需 token 也能成功

#### Scenario: 禁用模式下的流式请求
- **WHEN** access control 模式为 disabled
- **THEN** /api/events 与 WebSocket 连接无需 token

### Requirement: Token 保护的 API 边界
当 access control 模式为 token 时，系统 SHALL 要求 /api HTTP 路由提供
有效 token，并在未授权时返回 401，同时保持 /health 公开。

#### Scenario: 非 localhost 需要 token
- **WHEN** access control 模式为 token 且 allowLocalhostBypass 为 false
- **AND** /api 请求缺少有效 token
- **THEN** 系统返回 401，并携带 ApiResponse 错误负载

#### Scenario: Localhost bypass 生效
- **WHEN** access control 模式为 token 且 allowLocalhostBypass 为 true
- **AND** 请求来自 localhost 且未提供 token
- **THEN** 请求被接受

#### Scenario: 非 localhost 仍需 token
- **WHEN** access control 模式为 token 且 allowLocalhostBypass 为 true
- **AND** 请求来自非 localhost 且未提供 token
- **THEN** 系统返回 401，并携带 ApiResponse 错误负载

#### Scenario: Header 中的 token 被接受
- **WHEN** /api 请求包含 Authorization: Bearer <token>
- **OR** 请求包含 X-API-Token: <token>
- **THEN** 当 token 匹配时请求被授权

#### Scenario: Token 不匹配
- **WHEN** /api 请求携带的 token 与配置不匹配
- **THEN** 系统返回 401，并携带 ApiResponse 错误负载

#### Scenario: Health 端点保持公开
- **WHEN** access control 模式为 token
- **THEN** /health 请求无需 token 也能成功

### Requirement: SSE 与 WebSocket Token 校验
当 access control 模式为 token 时，系统 SHALL 要求 SSE 和 WebSocket
流提供 token，并在无法携带 header 时接受 query 参数。

#### Scenario: SSE 通过 query param 提供 token
- **WHEN** 客户端连接 /api/events 且带有 ?token=<token>
- **THEN** token 有效时 SSE 连接被接受

#### Scenario: SSE token 缺失或无效
- **WHEN** 客户端连接 /api/events 且 token 缺失或无效
- **THEN** 系统返回 401，并携带 ApiResponse 错误负载

#### Scenario: WS 通过 query param 提供 token
- **WHEN** 客户端连接 WebSocket 流且带有 ?token=<token>
- **THEN** 仅当 token 有效时才允许升级

#### Scenario: WS token 缺失或无效
- **WHEN** 客户端连接 WebSocket 流且 token 缺失或无效
- **THEN** 连接被拒绝并返回 401

### Requirement: 访问控制响应脱敏
系统 SHALL 在 UserSystemInfo/config 响应中脱敏 accessControl.token。

#### Scenario: Config 响应中脱敏 token
- **WHEN** 客户端获取 UserSystemInfo
- **THEN** 响应负载中省略或清空 accessControl token 字段

### Requirement: 客户端 Token 透传
前端 SHALL 在本地 token 已设置时，为 API、SSE 和 WebSocket 请求附带
对应的 token。

#### Scenario: Authorization header 被附加
- **WHEN** 客户端本地保存了 token
- **THEN** fetch 请求包含 Authorization: Bearer <token>

#### Scenario: Stream URL 包含 token
- **WHEN** 客户端本地保存了 token
- **THEN** SSE 与 WebSocket URL 包含 ?token=<token>

#### Scenario: 未设置 token 不注入
- **WHEN** 客户端本地未保存 token
- **THEN** fetch 请求不附加 Authorization，SSE/WS URL 不包含 token
