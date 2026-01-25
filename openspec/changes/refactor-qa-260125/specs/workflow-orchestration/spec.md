## ADDED Requirements
### Requirement: 工作流草稿保留
工作流视图 MUST 在刷新数据到达时保留未保存的 TaskGroup 草稿编辑，并且仅在用户保存或丢弃更改后替换草稿状态。

#### Scenario: 编辑中服务端刷新
- **WHEN** 工作流视图在用户有未保存编辑时收到更新的 TaskGroup 数据
- **THEN** UI 保留草稿编辑，并仅在保存或丢弃后呈现最新服务端状态
