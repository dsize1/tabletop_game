---
description: Node.js/TypeScript/Express 编码规范与约定
---

# Node 编码规范

- TypeScript + Express，ESM（`"type": "module"`）
- `apps/` 使用 npm workspaces（成员：`qq-gateway`、`web`）
- 目录结构：`src/index.ts` 为入口，按需拆 `src/routes`、`src/services`
- 环境变量走 `.env`（不入库，提供 `.env.example`）
- 与 Rust 侧通信：Redis Streams（`qq.raw` / `qq.push`）与 HTTP API（auth/资源查询）
