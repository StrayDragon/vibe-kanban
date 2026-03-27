## Context

VK 允许在 YAML 配置中使用 `{{env.*}}` / `{{secret.*}}` 做占位符注入，这对避免把 token 直接写进 YAML 很有价值。但目前模板解析对所有字符串字段递归生效，导致：
- “模板写到哪里都生效”的语义过于宽泛，难以维护安全边界。
- 一旦某字段被 API/日志/UI 传播，就可能把展开后的 secret 带出。

## Goals / Non-Goals

**Goals:**
- 明确模板解析白名单字段集合，并将其固化为可测试的契约。
- 非白名单字段出现模板时 fail-closed，错误指向具体字段路径。
- public config（用于 API/UI display）永不展开模板。

**Non-Goals:**
- 不升级模板语法为更强表达式语言。
- 不在本变更中调整 secret.env 的权限/owner 校验策略。

## Decisions

1. **解析从“YAML Value 全树递归”改为“结构化白名单字段展开”**
   - 选择：先将 YAML 解析为结构化 `Config` / `ProjectsFile`（不做模板展开），然后对明确字段做 `resolve_templates_in_string()`。
   - 原因：白名单以字段路径为单位更可控；也更容易输出准确的错误（路径、字段名）。
   - 备选：对 `serde_yaml::Value` 做“只遍历白名单 key 的子树”。实现复杂且容易漏掉/误扫。

2. **非白名单字段出现模板：作为配置错误**
   - 选择：在校验阶段扫描非白名单字段，若发现 `{{` 片段则报错（含字段路径）。
   - 原因：避免 silent footgun；让用户在本地及时发现潜在泄漏点。
   - 备选：保留原样（不展开且不报错）。会让配置看似生效但实际不生效，排障困难。

3. **白名单字段集合（v1）以“最小可用”为原则**
   - 选择：仅允许模板出现在以下类别字段：
     - token/credential：`github.pat`、`github.oauth_token`、`access_control.token`（以及未来新增的 credential 字段）
     - executor profiles 的 `env` map（用于注入运行时 token）
     - command/script：`projects[*].dev_script`、`projects[*].repos[*].setup_script/cleanup_script`、workspace lifecycle hook 的 `command`
   - 原因：这些字段天然可能需要 secret 注入且不会被 API 直出（public_config 保留占位符）。
   - 备注：白名单可在后续版本通过明确变更扩展，但必须同步更新 schema 文档与测试。

4. **防滥用限制**
   - 选择：对单字符串的模板展开次数、最终长度设置上限（例如最多 128 次替换、最终长度上限 64KiB），避免极端配置导致内存/CPU 放大。
   - 原因：模板解析是纯字符串替换，但仍应避免 DoS 型配置。

## Risks / Trade-offs

- [Breaking] 非白名单字段曾使用模板的用户会在升级后加载失败。
  - 缓解：错误信息明确字段路径，并提供迁移建议（例如把 secret 改放在允许字段或 executor env）。
- [白名单选取] 太窄会不够用，太宽会失去安全收益。
  - 缓解：先最小集合，后续按真实需求增量扩展，每次都通过 spec + tests 固化。

## Migration Plan

1. 引入白名单展开逻辑与“非白名单模板报错”校验。
2. 更新 schema description 与文档，列出允许字段与示例。
3. 增加回归测试覆盖允许/拒绝/优先级/上限。
4. 发布，并在 release notes 标记 BREAKING（模板范围收敛）。

## Open Questions

- executor profiles 中除 `env` 之外的字段（如额外参数）是否也需要模板？倾向先不支持，避免扩大边界。

