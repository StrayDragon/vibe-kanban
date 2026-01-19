## ADDED Requirements
### Requirement: 一致的错误状态映射
系统 SHALL 将 ApiError 变体映射为 HTTP 状态码，并在非 200 状态码下
返回 ApiResponse 错误负载。

#### Scenario: BadRequest 状态
- **WHEN** 输入校验失败
- **THEN** 响应状态为 400，且包含 ApiResponse 错误负载

#### Scenario: NotFound 状态
- **WHEN** 请求的资源不存在
- **THEN** 响应状态为 404，且包含 ApiResponse 错误负载

#### Scenario: Conflict 状态
- **WHEN** 请求与已有状态冲突
- **THEN** 响应状态为 409，且包含 ApiResponse 错误负载

#### Scenario: Unauthorized 状态
- **WHEN** 请求缺少必需的认证
- **THEN** 响应状态为 401，且包含 ApiResponse 错误负载

#### Scenario: Forbidden 状态
- **WHEN** 请求被判定为无权限访问
- **THEN** 响应状态为 403，且包含 ApiResponse 错误负载

#### Scenario: Internal Server Error 状态
- **WHEN** 发生未处理的服务端错误
- **THEN** 响应状态为 500，且包含 ApiResponse 错误负载
