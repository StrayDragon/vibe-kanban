## ADDED Requirements
### Requirement: 事务化任务与尝试创建
系统 SHALL 在任务与尝试的创建/启动流程中使用事务，覆盖 task、workspace
与 workspace_repo 的写入。

#### Scenario: 创建失败回滚
- **WHEN** 创建/启动流程中任一写入失败
- **THEN** 不应留下部分 task、workspace 或 workspace_repo 记录

#### Scenario: WorkspaceRepo 创建失败回滚
- **WHEN** workspace_repo 创建失败
- **THEN** task 与 workspace 记录不会被持久化

### Requirement: 启动失败时清理
当 start_workspace 在提交后失败时，系统 SHALL 在返回错误前清理由该流程
创建的 workspace 与 workspace_repo 记录。

#### Scenario: 启动失败清理 workspace 记录
- **WHEN** start_workspace 在提交后失败
- **THEN** 创建的 workspace 与 workspace_repo 记录被移除

#### Scenario: 启动失败返回错误
- **WHEN** start_workspace 在提交后失败且清理完成
- **THEN** API 返回错误，且数据库中无残留记录
