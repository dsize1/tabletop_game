---
name: rust-axum
description: Rust Axum 服务开发。使用于 crates/ 下各模块的路由、中间件、状态管理、HTTP API 相关任务。
---

# Rust Axum 开发

框架 Axum + tokio。约定见 `.pi/prompts/rust-conventions.md`。

- `crates/server` 为单进程多模块入口，组合各 lib crate
- 共享状态用 `Arc`；鉴权走 `auth` 模块
- HTTP API 负责：auth（token 签发/校验）、资源查询；健康检查 `/health`
