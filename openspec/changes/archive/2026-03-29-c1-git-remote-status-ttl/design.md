## Context

`/api/task-attempts/:attemptId/branch-status` 在 UI 侧属于高频查询：
- SSE 连接断开时会进入可见性驱动的 fallback polling（默认 5s）。
- 多 repo workspace 会在一次请求里对每个 repo 计算本地/远端状态。

当前实现中，远端 ahead/behind 通过 `GitService::get_remote_branch_status` 触发 `git fetch` 来刷新远端 tracking refs。并且 fetch refspec 采用 `+refs/heads/*:refs/remotes/<remote>/*`，会把远端所有 heads 拉取/写入，成本与 repo 的分支规模正相关，且在轮询下被放大为主要 CPU/IO/网络热点。

## Goals / Non-Goals

**Goals:**
- 将 branch-status 的远端 refresh 变为“按需最小 fetch + TTL 限流”
- 在 fetch 失败/慢的情况下仍能快速返回（允许使用现有 refs 计算得到的陈旧结果）
- 保持现有 API response shape 不变

**Non-Goals:**
- 不引入“全量 branch-status 缓存”（不缓存本地 dirty/冲突/merge 状态）
- 不新增前端强制刷新按钮或新 HTTP endpoint
- 不对其它 git 操作（rebase/merge 等）统一引入 TTL gate（仅覆盖 branch-status 的远端比较路径）

## Decisions

### Decision: TTL gate 放在 `GitService::get_remote_branch_status`
**原因**：该函数是远端 ahead/behind 的唯一入口，能集中控制副作用（fetch）。handler 层无需知道细节，避免重复实现。

**替代方案**：在 handler 层做缓存/批处理。缺点是逻辑分散，难以复用且更容易出现“调用者忘记限流”的回归。

### Decision: fetch refspec 仅抓取目标远端分支
将 refspec 从 `+refs/heads/*:refs/remotes/<remote>/*` 收敛为：
`+refs/heads/<branch>:refs/remotes/<remote>/<branch>`

**原因**：ahead/behind 只需要目标 base 分支；全量 heads fetch 对状态计算没有额外收益。

### Decision: in-progress 期间不阻塞等待
并发请求在同一个 `repo + remote + branch` 上遇到正在 fetch 时，不等待 fetch 完成，直接使用当前本地已有的远端 tracking refs 做比较，允许结果短暂陈旧。

**原因**：branch-status 是 UI 辅助信息，优先保证响应延迟与线程池健康；等待会把慢 fetch 放大为级联阻塞。

## Risks / Trade-offs

- [陈旧性] TTL 窗口内远端状态可能落后 → 通过可配置 TTL + push 成功路径的针对性 fetch 来缓解
- [失败冷却] 网络故障时仍可能周期性尝试 fetch → 增加失败冷却窗口，避免每次轮询都触发失败请求
- [状态表增长] per-key state 可能随着 repo 数增长 → key 数量与 workspace repo 数量同阶，且可接受；若后续需要可增加按 last_used 清理

## Migration Plan

无数据迁移；纯实现行为变更（降低副作用与资源占用）。回滚仅需恢复旧实现。

## Open Questions

无。

