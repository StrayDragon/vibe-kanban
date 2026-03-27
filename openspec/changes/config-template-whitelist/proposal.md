## Why

当前 YAML 模板解析（`{{secret.*}}` / `{{env.*}}`）会递归作用于**所有**字符串字段。这种“全局展开”带来几个问题：
- 安全性：用户如果把模板写进任何字符串字段（包括可能被 API 回传/被日志记录/被 UI 展示的字段），就可能把 secret 展开值带到不该出现的地方，形成数据外泄或放大暴露面。
- 可维护性：模板支持范围不清晰，后续字段演进时很难保证“不泄漏”边界不被破坏。
- 可预测性：模板在“看似普通的字符串字段”里也会生效，容易产生意外行为。

我们希望将模板解析收敛为**白名单字段**，并对非白名单字段出现模板做 fail-closed（明确报错），从语义层面降低泄露与误用风险。

## What Changes

- **BREAKING**: 模板解析只允许出现在明确白名单字段中（例如 token/env/command 类字段）；其它字段若包含 `{{...}}` 语法将导致配置校验失败并给出可操作错误（指出字段路径与迁移建议）。
- 将“模板允许字段”显式文档化（schema description + docs），并为允许字段补齐示例。
- 增加回归测试：\n  - 白名单字段模板可解析且遵循 `secret.env > system env` 优先级\n  - 非白名单字段出现模板会被拒绝\n  - public config 视图永不展开模板（保持占位符文本）

Goals:
- 让模板解析能力“最小化且明确”：只支持必要字段，语义稳定。
- fail-closed：避免模板悄悄出现在不该出现的字段，降低泄漏面与隐式副作用。
- 通过 schema + 测试固化边界，减少后续维护成本。

Non-goals:
- 不改变模板语法（仍是 `{{env.NAME}}/{{env.NAME:-default}}/{{secret.NAME}}`）。
- 不引入更复杂的模板语言（无表达式/函数/循环等）。
- 不改变 secret.env 权限与加载策略（本变更只关注“模板允许范围”）。

Risks:
- [BREAKING] 现有用户可能在非白名单字段使用了模板，升级后会加载失败。
  - Mitigation: 错误信息给出字段路径 + 推荐替代方案；提供迁移指引与示例。
- 白名单选择不当会影响实际用例（字段太少导致不够用；字段太多导致边界不清）。
  - Mitigation: 首次版本以“token/env/command”最小集合为主；允许后续通过明确版本变更扩展。

Verification:
- `cargo test -p config`
- `cargo test -p app-runtime reload`

## Capabilities

### New Capabilities
<!-- 无 -->

### Modified Capabilities
- `yaml-user-config`: 模板解析范围从“所有字符串字段”改为“明确白名单字段”；非白名单出现模板时 fail-closed。

## Impact

- `crates/config/src/lib.rs`（模板解析与校验逻辑）
- `crates/config/src/schema.rs`（字段说明、模板支持文档化）
- `crates/app-runtime/src/lib.rs`（错误可观测性/状态提示，可能需要调整报错文案）
- 前端 Settings/文档提示（如需更新模板使用说明）

