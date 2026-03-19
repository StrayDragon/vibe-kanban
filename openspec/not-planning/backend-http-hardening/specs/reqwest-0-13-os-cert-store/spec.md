## ADDED Requirements

### Requirement: 出站 HTTPS 验证咨询 OS trust store

后端发起的所有出站 HTTPS 请求应当（SHALL）使用一个会咨询 operating system trust store 的 verifier 来验证服务端证书。

该行为必须（MUST）在企业环境中可用：例如 OS trust store 安装了额外根证书（corporate proxies 常见）时也能正常工作。

#### Scenario: OS trust store 存在企业根证书

- **WHEN** OS trust store 中包含 network 所需的额外 root CA
- **THEN** 出站 HTTPS 请求应成功，无需为应用单独配置自定义 CA

### Requirement: 使用单一 `reqwest` major version

后端应当（SHALL）避免混用多个 `reqwest` major versions，以防止不同 crates 间出现不一致的 HTTP/TLS 行为。

#### Scenario: 依赖图使用 `reqwest` 0.13

- **WHEN** 检查 workspace dependency graph（例如使用 `cargo tree`）
- **THEN** `reqwest` 解析为单一 major version（`0.13.x`）
