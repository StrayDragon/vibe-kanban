## ADDED Requirements
### Requirement: Task Group 指令持久化
系统 SHALL 在创建或更新任务组图时持久化可选的 TaskGroupNode.instructions 数据。

#### Scenario: 更新后指令被保留
- **WHEN** 任务组图更新时包含节点指令
- **THEN** 后续读取返回相同的指令内容

#### Scenario: 未提供指令保持为空
- **WHEN** 节点未包含 instructions
- **THEN** 后续读取仍为空或缺失该字段

### Requirement: Task Group 提示增强
系统 SHALL 在从带有非空指令的任务组节点启动尝试时，将该指令追加到任务提示词。

#### Scenario: 指令被追加到提示词
- **WHEN** 从包含指令的节点启动任务尝试
- **THEN** 初始提示词包含该指令内容

#### Scenario: 缺少指令不影响提示词
- **WHEN** 从没有指令的节点启动任务尝试
- **THEN** 初始提示词与原任务提示一致

#### Scenario: 空指令不追加
- **WHEN** 节点 instructions 为空字符串或仅包含空白
- **THEN** 初始提示词与原任务提示一致

### Requirement: Task Group 指令编辑 UI
UI SHALL 在 Task Group 工作流中提供节点指令编辑方式。

#### Scenario: 在工作流 UI 中编辑指令
- **WHEN** 用户在 Task Group 工作流中编辑节点指令
- **THEN** 更新后的指令保存到任务组图

#### Scenario: 在工作流 UI 中清空指令
- **WHEN** 用户在 Task Group 工作流中清空节点指令
- **THEN** 该节点保存为空指令
