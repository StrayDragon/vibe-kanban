## 1. JSON 长度估算器（去除热路径序列化）

- [x] 1.1 在 `crates/logs-protocol` 增加无堆分配的 JSON 长度估算（`serde_json::Value` + `json_patch::Patch`），并补齐单测（对比 `serde_json::to_string(...).len()`，确保不低估）
- [x] 1.2 将 `LogMsg::approx_bytes()` 的 `JsonPatch` 分支改为使用新估算器（移除运行时 `serde_json::to_string(patch)`）

## 2. MsgStore broadcast 与锁竞争优化

- [x] 2.1 在 `crates/logs-store` 引入 `VK_LOG_BROADCAST_CAPACITY` 配置项并应用到 sequenced/raw/normalized broadcast 通道（默认值下调）
- [x] 2.2 精简 `MsgStore` 冗余通道/冗余拷贝（统一基于 sequenced 流派生 `LogMsg` 流），并把 patch 提取/stdout-stderr clone 尽量移到锁外
- [x] 2.3 为 `LogMsg` 流补齐 lagged resync（基于 `seq` 去重与回放保留历史），避免静默缺失；更新相关调用点（executors/event streams/server 内部）

## 3. entry-json 字节预算估算替换

- [x] 3.1 将 `logs-store` 的 entry-json 大小估算从 `serde_json::to_string(value)` 替换为无分配估算器，并新增单测覆盖典型 `Value`（包含转义字符串/嵌套对象）

## 4. 验证与回归

- [x] 4.1 运行并修复：`cargo test -p logs-protocol -p logs-store`
- [x] 4.2 运行并修复：`just qa`、`just openspec-check`
