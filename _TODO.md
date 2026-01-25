# Code Quality Issues Report

> QA审查日期: 2026-01-25
> 审查范围: 前端 (React/TypeScript) + 后端 (Rust)

---

## 目录

- [前端问题](#前端问题)
  - [高优先级](#前端-高优先级)
  - [中优先级](#前端-中优先级)
  - [低优先级](#前端-低优先级)
- [后端问题](#后端问题)
  - [高优先级](#后端-高优先级)
  - [中优先级](#后端-中优先级)
  - [低优先级](#后端-低优先级)
- [通用问题](#通用问题)

---

## 前端问题

### 前端-高优先级

#### 1. 过大的组件文件

**问题**: 多个组件文件过长，违反单一职责原则，难以维护和测试。

| 文件 | 行数 | 建议 |
|------|------|------|
| `frontend/src/components/tasks/TaskFollowUpSection.tsx` | 910行 | 拆分为多个子组件 |
| `frontend/src/hooks/useConversationHistory.ts` | 1177行 | 拆分逻辑到多个hooks |
| `frontend/src/components/NormalizedConversation/DisplayConversationEntry.tsx` | 898行 | 按entry类型拆分渲染逻辑 |
| `frontend/src/lib/api.ts` | 1112行 | 按资源类型拆分API模块 |

**改进建议**:
- `TaskFollowUpSection.tsx`: 将队列管理、冲突处理、变体选择等逻辑拆分为独立hooks
- `useConversationHistory.ts`: 将历史加载、流处理、状态管理拆分为独立hooks
- `DisplayConversationEntry.tsx`: 将各类entry渲染器（ToolCallCard、PlanPresentationCard等）拆分为独立文件
- `api.ts`: 按资源拆分为 `api/projects.ts`, `api/tasks.ts`, `api/attempts.ts` 等

#### 2. console.log 残留 (34处)

**问题**: 生产代码中存在 `console.log/warn/error` 调用，应使用统一日志系统。

**受影响文件**:
- `frontend/src/lib/api.ts` (4处)
- `frontend/src/hooks/useTaskMutations.ts` (4处)
- `frontend/src/i18n/config.ts` (5处)
- `frontend/src/hooks/useDevServer.ts` (2处)
- 其他20+处

**改进建议**:
- 创建统一的日志工具类
- 开发环境使用 `console.*`，生产环境使用 no-op 或发送到日志服务
- 使用 ESLint 规则 `no-console` 配合 CI 检查

#### 3. `any` 类型使用 (19处)

**问题**: 使用 `any` 或 `as any` 绕过类型检查，削弱TypeScript的类型安全。

**受影响文件**:
- `frontend/src/components/tasks/TaskFollowUpSection.tsx` (3处)
- `frontend/src/pages/settings/AgentSettings.tsx` (3处)
- `frontend/src/components/dialogs/tasks/RestoreLogsDialog.tsx` (7处)

**改进建议**:
- 定义具体的类型接口
- 使用 `unknown` 配合类型守卫
- 必要时使用泛型而非 `any`

### 前端-中优先级

#### 4. Hooks 依赖项过多

**问题**: 多个hooks的依赖数组过长，导致频繁重渲染和难以追踪的bug。

**示例** (`useConversationHistory.ts`):
```typescript
useEffect(() => {
  // ...
}, [
  attempt.id,
  idStatusKey,
  emitEntries,
  ensureProcessVisible,
  loadRunningAndEmitWithBackoff,
]);
```

**改进建议**:
- 使用 `useReducer` 替代多个 `useState`
- 将复杂逻辑抽取到自定义hooks
- 考虑使用状态管理库如 Zustand (项目已使用)

#### 5. 过度使用 useCallback/useMemo

**问题**: `TaskFollowUpSection.tsx` 中有大量 `useCallback` 和 `useMemo`，部分可能是不必要的优化。

**改进建议**:
- 使用性能分析工具确认瓶颈
- 仅对确实需要稳定引用的回调使用 `useCallback`
- 考虑使用 React Compiler (React 19+) 自动优化

#### 6. 状态管理分散

**问题**: 状态分散在多个 Context 和 hooks 中，缺乏统一管理。

**当前 Context**:
- `ProjectContext`
- `EventStreamContext`
- `SearchContext`
- `EntriesContext`
- `RetryUiContext`
- `ReviewProvider`
- `ClickedElementsProvider`

**改进建议**:
- 评估是否可以合并相关Context
- 使用 Zustand stores 替代部分 Context
- 文档化数据流向

#### 7. 缺少组件懒加载

**问题**: 大型组件未使用懒加载，影响首屏加载性能。

**改进建议**:
```typescript
// 当前
import { TaskGroupWorkflow } from '@/pages/TaskGroupWorkflow';

// 改进
const TaskGroupWorkflow = lazy(() => import('@/pages/TaskGroupWorkflow'));
```

### 前端-低优先级

#### 8. CSS类名重复

**问题**: 相似的Tailwind类组合在多处重复。

**示例**:
```tsx
// 多处重复
className="flex items-center gap-2 text-sm text-muted-foreground"
```

**改进建议**:
- 使用 `@apply` 创建复用的样式类
- 或创建组件封装常用样式

#### 9. TODO/FIXME 注释 (1处前端)

**位置**: `frontend/src/hooks/useLayoutMode.ts:13`
```typescript
// TODO: Remove this redirect after v0.1.0 (legacy URL support for bookmarked links)
```

**建议**: 创建Issue追踪，版本发布后移除

---

## 后端问题

### 后端-高优先级

#### 1. unwrap() 滥用 (623处)

**问题**: 大量使用 `unwrap()` 可能导致panic，在生产环境中不可接受。

**高风险文件**:
| 文件 | 数量 |
|------|------|
| `crates/local-deployment/src/copy.rs` | 61处 |
| `crates/server/src/routes/task_attempts.rs` | 49处 |
| `crates/services/src/services/diff_stream.rs` | 21处 |
| `crates/executors/src/executors/droid/normalize_logs.rs` | 20处 |
| `crates/executors/src/executors/claude.rs` | 24处 |

**改进建议**:
```rust
// 当前
let value = some_option.unwrap();

// 改进
let value = some_option.ok_or_else(|| ApiError::BadRequest("Expected value"))?;
// 或
let value = some_option.unwrap_or_default();
```

#### 2. expect() 使用 (247处)

**问题**: `expect()` 比 `unwrap()` 稍好但仍会panic。

**高风险文件**:
| 文件 | 数量 |
|------|------|
| `crates/services/tests/git_ops_safety.rs` | 67处 |
| `crates/executors/src/executors/codex/normalize_logs.rs` | 15处 |
| `crates/executors/src/executors/codex/session.rs` | 16处 |
| `crates/services/src/services/events/patches.rs` | 25处 |

**改进建议**:
- 测试代码中可保留 `expect()`
- 生产代码应使用 `?` 操作符和适当的错误处理

#### 3. 过大的文件

**问题**: 某些文件职责过多，难以维护。

| 文件 | 行数 | 建议 |
|------|------|------|
| `crates/server/src/routes/task_attempts.rs` | 2627行 | 按功能拆分模块 |
| `crates/services/src/services/container.rs` | 2159行 | 拆分trait实现 |

**改进建议**:
- `task_attempts.rs`: 将测试移到单独文件，将helpers拆分到util模块
- `container.rs`: 将日志backfill、清理逻辑拆分为独立服务

### 后端-中优先级

#### 4. clone() 过度使用 (908处)

**问题**: 大量 `clone()` 调用可能影响性能。

**高频文件**:
| 文件 | 数量 |
|------|------|
| `crates/executors/src/executors/droid/normalize_logs.rs` | 71处 |
| `crates/executors/src/executors/codex/normalize_logs.rs` | 63处 |
| `crates/executors/src/executors/claude.rs` | 58处 |
| `crates/services/src/services/container.rs` | 38处 |
| `crates/local-deployment/src/container.rs` | 42处 |

**改进建议**:
- 使用 `Arc<T>` 共享不可变数据
- 使用 `Cow<'a, T>` 避免不必要的克隆
- 考虑使用引用而非拥有值

#### 5. TODO/FIXME 注释 (5处)

**位置及内容**:

| 文件 | 行号 | 内容 |
|------|------|------|
| `crates/server/src/routes/config.rs` | 91 | `// TODO: update frontend, BE schema has changed` |
| `crates/executors/src/executors/claude.rs` | 593 | `// TODO: Add proper ToolResult support` |
| `crates/services/src/services/git/cli.rs` | 26 | `// TODO: make GitCli async` |
| `crates/services/src/services/filesystem_watcher.rs` | 152 | `// FIXME: capture file-type information earlier` |
| `crates/services/src/services/github/cli.rs` | 132 | `// TODO: support writing the body to a temp file` |

**建议**: 创建Issue追踪这些技术债务

#### 6. 错误处理不一致

**问题**: `ApiError` 的映射逻辑分散，某些错误类型返回 500 而非更具体的状态码。

**示例** (`crates/server/src/error.rs`):
```rust
ApiError::Session(_) => (StatusCode::INTERNAL_SERVER_ERROR, "SessionError"),
```

**改进建议**:
- 为 Session 错误添加更细粒度的状态码映射
- 考虑实现 `From<XxxError> for ApiError` 时携带更多上下文

#### 7. 日志级别不一致

**问题**: 某些地方用 `tracing::error!` 记录非关键错误，某些用 `tracing::warn!`。

**改进建议**:
- 制定日志级别指南
- `error!`: 需要立即关注的问题
- `warn!`: 异常但可恢复的情况
- `info!`: 重要业务事件
- `debug!`: 开发调试信息

### 后端-低优先级

#### 8. 测试代码混在生产代码中

**问题**: `crates/server/src/routes/task_attempts.rs` 文件末尾有大量测试代码（约700行）。

**改进建议**:
- 将测试移至 `tests/` 目录
- 或使用单独的 `task_attempts_tests.rs` 文件

#### 9. 硬编码常量

**问题**: 存在魔法数字和硬编码字符串。

**示例**:
```rust
// crates/services/src/services/container.rs
const DEFAULT_LOG_BACKFILL_CONCURRENCY: usize = 4;
const LOG_EVERY_PROCESSES: usize = 25;
const LOG_EVERY_BYTES: i64 = 100 * 1024 * 1024;
```

**改进建议**:
- 将可配置项移至配置文件
- 集中管理常量定义

---

## 通用问题

### 1. 缺乏统一的错误日志格式

**问题**: 前后端错误日志格式不一致，难以做关联分析。

**改进建议**:
- 定义统一的错误日志结构
- 包含: timestamp, request_id, error_code, message, stack_trace
- 后端返回错误时携带 trace_id

### 2. API设计一致性

**问题**: 部分API返回 `ApiResponse<T>` 包装，部分直接返回数据。

**改进建议**:
- 统一API响应格式
- 文档化API设计规范

### 3. 缺少集成测试

**问题**: 前端缺乏E2E测试，后端集成测试覆盖不足。

**改进建议**:
- 前端: 添加Playwright/Cypress E2E测试
- 后端: 增加API集成测试覆盖

### 4. 依赖版本管理

**问题**: 部分依赖可能需要更新。

**改进建议**:
- 定期运行 `pnpm outdated` 和 `cargo outdated`
- 建立依赖更新策略

---

## 总结统计

| 类别 | 高优先级 | 中优先级 | 低优先级 |
|------|----------|----------|----------|
| 前端 | 3 | 4 | 2 |
| 后端 | 3 | 4 | 2 |
| 通用 | - | 4 | - |

**建议处理顺序**:
1. 后端 `unwrap()` 问题 (安全风险)
2. 前端组件拆分 (可维护性)
3. 移除 `console.log` (专业性)
4. 后端大文件拆分 (可维护性)
5. 其他中低优先级问题

---

*此报告由QA审查生成，请根据项目实际情况调整优先级。*
