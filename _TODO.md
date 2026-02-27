- 现有 MCP（mcp_task_server）工具只有 10 个：list_projects/list_tasks/create_task/get_task/update_task/delete_task/list_repos/start_task_attempt/
list_task_attempts/follow_up；能做“建任务-开 attempt-跟进/取消”，但缺少“进度/日志/产物”闭环。
- 建议补齐 MCP 的“机器编排最小闭环”能力：
  - executors.list（可用 agent/版本/能力/ACP 支持）
  - attempt.get / attempt.status（attempt 状态、当前 session、最后活动时间、失败原因）
  - attempt.logs.tail（cursor/limit，避免 Zirvox 轮询全量）
  - attempt.events.subscribe（返回 SSE/WS 地址或订阅 token；或直接在 MCP 支持事件推送）
  - workspace.diff.get / workspace.patch.get（worktree diff、变更摘要、关键文件列表）
  - task.search（按 title/tag/status 搜索，方便 Zirvox 做“自动跟进/召回”）
- WS/SSE/API 互联方案建议：
  - MCP 保持“命令式工具面”，新增稳定的 /events（SSE）或 /ws（WS）事件流做“可观测/跟进”；MCP 工具返回订阅信息即可。
  - 若要做 Zirvox↔Kanban 的长期协议，建议“版本化 + 幂等”：每个 mutating API 支持 request_id 去重；所有事件带 correlation_id（task_id/attempt_id/session_id/
    run_id）。
  - 不要依赖 MCP 进程的 cwd 推断上下文：所有工具都应显式接受 project_id/task_id/attempt_id（便于 Zirvox 作为外部 orchestrator 调用）。
- 安全/部署：至少加可选 token（本地默认可关），为未来“Zirvox 独立进程/远程”留后路。
