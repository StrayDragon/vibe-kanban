## Why

当前 `/api/task-attempts/:attemptId/branch-status` 会在每个 repo 上计算本地状态与远端 ahead/behind。其中远端部分会触发 `git fetch` 更新远端 tracking refs；在 SSE 断开时，前端会按 5s 轮询 branch-status，从而把 `git fetch` 放大为高频网络/CPU/IO 热点（尤其是多 repo workspace）。

这会带来：
- 明显的 CPU/IO 消耗与 UI 卡顿（spawn_blocking 线程池被 git 占满）
- 不必要的网络请求，可能触发远端限流/失败重试风暴
- 在“只想看状态”的场景下引入过重的副作用

## What Changes

- 后端为 branch-status 相关的远端 tracking refs 刷新引入 TTL 限流：同一 `repo + remote + branch` 在 TTL 内最多执行一次 `git fetch`。
- `git fetch` refspec 从“抓取远端所有 heads”收敛为“仅抓取需要比较的远端分支”，避免全量 refs 扫描与写入。
- 远端刷新失败时降级：仍返回本地状态；远端字段允许为 `null`；并对失败做短暂冷却以避免高频重复 fetch。
- 增加可配置环境变量：`VK_GIT_REMOTE_STATUS_FETCH_TTL_SECS`（默认值见设计文档）。
- 不改变现有 HTTP API 路径与 response JSON shape。

## Capabilities

### New Capabilities
- `git-remote-status-ttl`: Branch status 查询在需要远端 ahead/behind 时，对远端 refs 刷新做 TTL 限流并最小化 fetch 范围，以降低 CPU/IO/网络开销。

### Modified Capabilities
- （无）

## Impact

- Backend
  - `crates/repos/src/git/mod.rs`: `get_remote_branch_status` 的 fetch 行为与 TTL gate
  - `crates/server/src/routes/task_attempts/handlers.rs`: branch-status 调用链（不改 response shape）
- Tests
  - `crates/repos`: 添加本地 git 仓库 + 本地 bare remote 的回归测试覆盖（确保 TTL 生效、refspec 收敛）

## Goals

- 在不改 API shape 的前提下，显著减少 branch-status 引发的远端网络/CPU/IO 消耗
- 让远端 ahead/behind 计算在“可接受的新鲜度窗口（TTL）”内保持正确
- 避免 fetch 失败时的重试风暴（降低失败路径的放大效应）

## Non-goals

- 不对 branch-status 的其它本地字段（冲突检测、dirty 检测等）做缓存/合并优化
- 不新增 UI “强制刷新远端状态”入口或新 API（若需要单独立项）
- 不修改 `useSsePollingInterval` 的轮询策略

## Risks

- [风险] 远端状态在 TTL 窗口内可能短暂过期 → [缓解] TTL 可配置；push 成功路径已做针对性 fetch 刷新远端 tracking ref
- [风险] 远端分支解析错误导致 fetch 未覆盖目标 ref → [缓解] 单测覆盖带斜杠的分支名与无 upstream 的降级路径
- [风险] 并发请求在 TTL 边界产生少量重复 fetch → [缓解] per-key in-progress gate，in-progress 期间直接使用现有 refs 计算（允许短暂陈旧）

## Verification

- `cargo test -p repos`
- `cargo test -p server`
- `just qa`
- `just openspec-check`

