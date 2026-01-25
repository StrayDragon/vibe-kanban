## ADDED Requirements
### Requirement: 无执行进程时清空对话加载
UI SHALL 在没有可加载执行进程时清空对话加载状态。

#### Scenario: 空进程列表清空加载
- **WHEN** 执行进程列表为空且加载已完成
- **THEN** 对话历史加载状态为 false

#### Scenario: 加载进行中不提前清空
- **WHEN** 执行进程列表为空但仍在加载中
- **THEN** 对话历史加载状态保持为 true

### Requirement: 日志归一化韧性
系统 MUST 在日志序列异常或乱序时不发生 panic，并 SHALL 在继续流式处理前发出归一化错误条目。

#### Scenario: 工具结果异常
- **WHEN** 工具结果到达但没有匹配的待处理工具调用，或索引状态缺失
- **THEN** 流发出描述异常的错误条目，并继续处理后续事件

### Requirement: UI 中日志条目的稳定身份
UI MUST 使用稳定标识符（entry index 或 patch key）渲染原始与归一化日志条目，以避免在历史前置加载或截断时出现错误关联。

#### Scenario: 前置加载更早历史
- **WHEN** 用户加载更早的历史并将条目前置插入
- **THEN** 既有渲染条目保持身份且滚动位置稳定
