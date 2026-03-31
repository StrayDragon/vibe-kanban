## 1. Projects overlay（追加 repo 配置）

- [x] 1.1 确定 overlay 文件名与路径 helper（例如在 `utils-core` 增加 `vk_projects_ui_yaml_path()`），并在 `GET /api/config/status` 里可选暴露该路径用于排障
- [x] 1.2 在 `crates/config` 读取 overlay YAML（可选文件），并在 public/runtime config 构建时应用到 projects 列表
- [x] 1.3 实现 overlay 合并规则：按 `project_id` 追加 repos、同项目内按 normalized path 去重、缺失 project_id 给出明确错误
- [x] 1.4 为 overlay 合并添加单元测试（至少覆盖：成功追加、重复 path 幂等、overlay 引用不存在项目时报错）

## 2. 后端 API：按路径添加仓库

- [x] 2.1 定义 `POST /api/projects/{project_id}/repositories`（或新增专用路由）的请求/响应结构（包含 `path`、可选 `display_name`、可选 `reload`）
- [x] 2.2 实现 handler：校验路径存在且为目录、解析为绝对路径（可选 Git repo root 探测）、原子写入 overlay YAML、触发 `deployment.reload_user_config()`
- [x] 2.3 错误与幂等：重复 path 返回 no-op 或明确 duplicate 错误；写入失败/权限不足/项目不存在有清晰错误信息
- [x] 2.4 添加后端测试（至少覆盖：成功写入 + reload、无效路径、重复路径、overlay 写入失败时的错误返回）

## 3. 前端：Kanban 项目下拉“添加仓库…”

- [x] 3.1 在 Kanban 项目下拉中新增“添加仓库…”入口，并在选中项目上下文中可用
- [x] 3.2 实现弹窗：path 输入 + 基本校验 + 提交调用后端 API；成功/失败使用统一 toast/alert 提示
- [x] 3.3 成功后刷新项目仓库列表（通过 projects stream 自动刷新或主动 refetch），确保新增 repo 立即可见
- [x] 3.4 UI 冒烟验证：新增 repo 后可在创建/启动任务时选择，并能正常进入 workspace 流程

## 4. 前端：移除“+”二级菜单，统一创建弹窗

- [x] 4.1 定位 Kanban 中所有会弹出“新建任务/新建任务组”菜单的“+”入口，并改为点击直接打开创建弹窗
- [x] 4.2 在创建弹窗内加入“任务 / 任务组”类型切换（默认任务），并确保两种创建仍可用
- [x] 4.3 删除/下线不再使用的二级菜单组件与相关逻辑，确保键盘/可访问性行为不回退
- [x] 4.4 UI 冒烟验证：各处“+”点击后直接进入弹窗；创建任务与任务组都成功

## 5. 验证与回归

- [x] 5.1 运行前端检查：`pnpm run check` + `pnpm run lint`
- [x] 5.2 运行后端检查：`pnpm run backend:check` + `cargo test --workspace`
- [x] 5.3 本地联调：`pnpm run dev`，验证“添加仓库（overlay 写入 + reload）”与“创建入口单击直达弹窗”
