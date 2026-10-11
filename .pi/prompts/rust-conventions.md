---
description: Rust/Axum 编码规范与约定
---

# Rust 编码规范

- 框架：**Axum** + tokio；序列化：serde/serde_json；日志：tracing
- 错误处理：anyhow（应用层）/ thiserror（库层）
- crate 划分：`shared`（类型）、各模块为 lib、`server` 为 bin（单进程组合）
- 内部事件类型统一放 `crates/shared`
- 模块间通过 trait 接口解耦，`server` 负责组装
- 异步 Redis/SQLite 客户端按需引入，连接配置走环境变量（.env 不入库）
