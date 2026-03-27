## 1. 定义模板白名单字段集合（最小可用）

- [ ] 1.1 在 `crates/config` 定义“允许模板解析的字段”清单（以结构化字段路径表达，不依赖 YAML key 遍历）
- [ ] 1.2 更新 `crates/config/src/schema.rs` 的字段描述：明确哪些字段支持 `{{env.*}}/{{secret.*}}`，并给出示例
- [ ] 1.3 文档/提示同步：Settings/README 中模板用法只指向白名单字段

Verification:
- `cargo test -p config`

## 2. 实现白名单模板展开（结构化展开 + 非白名单 fail-closed）

- [ ] 2.1 将模板展开从“YAML Value 全树递归”改为“解析为结构化 Config 后对指定字段展开”
- [ ] 2.2 对非白名单字段出现 `{{...}}` 的情况报错：错误信息必须包含字段路径与迁移建议
- [ ] 2.3 public config 视图保持不展开（保留占位符文本），并补齐回归测试

Verification:
- `cargo test -p config`
- `cargo test -p app-runtime reload`

## 3. 防滥用与回归测试

- [ ] 3.1 为模板展开增加安全上限（替换次数、输出长度），避免极端配置导致资源放大
- [ ] 3.2 增加测试覆盖：白名单字段解析成功；非白名单字段拒绝；secret.env 优先级；缺失变量（含 default/无 default）；上限触发报错

Verification:
- `cargo test -p config`
