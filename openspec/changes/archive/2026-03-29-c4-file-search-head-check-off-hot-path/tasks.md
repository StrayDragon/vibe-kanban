## 1. Cache budget 与 GitService 最小化封装

- [x] 1.1 增加 cache budget `file_search_head_check_ttl`（env: `VK_FILE_SEARCH_HEAD_CHECK_TTL_SECS`），并在 app runtime 启动日志中输出该预算值
- [x] 1.2 在 `GitService` 增加只读 `get_head_oid()`（内部仅 `rev-parse HEAD`），并让文件搜索缓存构建使用该方法（验证：`cargo check -p repos`）

## 2. FileSearchCache 异步 HEAD 校验与热路径瘦身

- [x] 2.1 为 `FileSearchCache` 增加 HEAD 校验队列/去重状态/TTL 记录，并实现后台 worker：对比缓存 `head_sha` 与当前 `HEAD` OID，变化时 enqueue 重建
- [x] 2.2 更新 `FileSearchCache::search()`：cache hit 直接返回并按需调度后台 HEAD 校验；cache miss 维持现有 enqueue build + `CacheError::Miss`（验证：新增/更新单测覆盖）

## 3. 测试与验收

- [x] 3.1 新增 tokio 测试：HEAD 变化后首次 search 不同步 miss；后台刷新后 cache 的 `head_sha` 更新为新值（验证：`cargo test -p repos`）
- [x] 3.2 运行验收命令并修复直到通过：`just qa`、`just openspec-check`
