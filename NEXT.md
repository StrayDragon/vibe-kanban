# 下一步开发方向（基于最近 commits + OpenSpec）

最近的主线提交集中在：SeaORM 迁移（为多数据库做准备）、MCP 体验提升、smart agent version selection、配置迁移重做与 UI 稳定性修复。当前 repo 的“下一步”建议以 **稳定性与护栏** 为主，先把运行时风险和可观测性补齐，再推进更多功能迭代。

• - 目前 NEXT.md 已没有待处理事项（NEXT.md:7、NEXT.md:10）。
  - 额外建议人工确认一次：SQLite 连接默认用了 mode=rwc（crates/db/src/lib.rs:24），是否符合你们对“自动创建 db.sqlite”的预期。
