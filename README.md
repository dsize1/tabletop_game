# 桌游助手（QQ Bot Board Game Assistant）

基于 **QQ 群机器人**的桌游助手：接入 QQ 群消息，路由到多个桌游引擎，并通过 Web **独立站点**展示游戏公共资源与玩家个人资源。

## 模块

| 模块 | 位置 | 语言 | 职责 |
|---|---|---|---|
| qq-gateway | `apps/qq-gateway` | Node/TS | QQ 群协议接入：收群事件 / 发群消息 |
| web | `apps/web` | Node/TS | Express 独立站点：公共/个人资源展示 |
| bot-adapter | `crates/bot-adapter` | Rust | QQ 群事件 → 内部消息事件 |
| command-router | `crates/command-router` | Rust | 指令解析与路由 |
| game-engine-core | `crates/game-engine-core` | Rust | 引擎框架（EnginePlugin trait / 注册表） |
| engines/las-vegas | `crates/engines/las-vegas` | Rust | 首个桌游引擎「拉斯维加斯」（空） |
| auth | `crates/auth` | Rust | 短期 token 签发/校验、资源归属鉴权 |
| push | `crates/push` | Rust | 全局广播 + 个人定向推送 |
| server | `crates/server` | Rust | 单进程多模块入口（Axum HTTP API） |
| shared | `crates/shared` | Rust | 内部事件与公共类型 |

## 技术栈

- Node.js (TypeScript) + Express
- Rust + Axum（单进程多模块）
- Redis（Streams 消息总线 + 缓存）、SQLite（持久化）
- npm workspaces + Cargo workspace

## 目录结构

见 `AGENTS.md` 索引与 `docs/architecture.md`。

## 快速开始

前置：Node.js >= 18、Rust 工具链、Docker Desktop（WSL2 后端）。
国内拉镜像前先配镜像加速，详见 `docs/dev-environment.md`。

```bash
# 1. 起 Redis
docker compose up -d
docker exec tabletop-game-redis redis-cli ping   # 应返回 PONG

# 2. 编译 Rust 服务（server + game-engine-mcp）
cd crates && cargo build

# 3. 跑 server（bot-adapter + 健康自检 + /health）
cd crates && cargo run -p server

# 4. 跑 agent-gateway（消费 internal → 决策 → 聊天/猜拳，spawn game-engine-mcp）
cd apps/agent-gateway && npm run dev

# 5. 跑 qq-gateway（QQ 群接入：入站 + 出站）
cd apps/qq-gateway
cp .env.example .env            # 填入 QQBOT_APP_ID / QQBOT_APP_SECRET
npm run dev                     # 看到 “WebSocket 网关已连接” 即成功

# 6. 健康检查
curl http://127.0.0.1:3000/health
node apps/scripts/status.mjs
```

> 完整启动顺序与环境变量见 `docs/dev-environment.md` §6。

## 开发约定

见 `AGENTS.md`、`docs/`、`.pi/prompts/`、`.pi/skills/`。
