## ADDED Requirements
### Requirement: 无执行进程时清空对话加载
UI SHALL 在没有可加载执行进程时清空对话加载状态。

#### Scenario: 空进程列表清空加载
- **WHEN** 执行进程列表为空且加载已完成
- **THEN** 对话历史加载状态为 false

#### Scenario: 加载进行中不提前清空
- **WHEN** 执行进程列表为空但仍在加载中
- **THEN** 对话历史加载状态保持为 true
