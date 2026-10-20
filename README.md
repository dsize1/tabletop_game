# 桌游助手（QQ Bot Board Game Assistant）

基于 **QQ 群机器人**的桌游助手：接入 QQ 群消息，路由到多个桌游引擎，并通过 Web **独立站点**展示游戏公共资源与玩家个人资源。

## 模块

| 模块 | 位置 | 语言 | 职责 |
|---|---|---|---|
| qq-gateway | `apps/qq-gateway` | Node/TS | QQ 群协议接入：收群事件 / 发群消息 |
| web | `apps/web` | Node/TS | Express 独立站点：公共/个人资源展示 |
| bot-adapter | `crates/bot-adapter` | Rust | QQ 群事件 → 内部消息事件 |
| game-engine-core | `crates/game-engine-core` | Rust | 引擎框架（EnginePlugin trait / 注册表） |
| engines/las-vegas | `crates/engines/las-vegas` | Rust | 首个桌游引擎「拉斯维加斯」（6 轮 + 16 小游戏） |
| auth | `crates/auth` | Rust | 短期 token 签发/校验、资源归属鉴权 |
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
# 一键启动（推荐）：Redis + cargo build + server + decision + web + agent-gateway + qq-gateway
npm run dev:up

# 停止（含 Redis 容器；加 --keep-redis 只停应用）
npm run dev:down
```

> 以上在**仓库根目录**执行。也可 `cd apps && npm run dev:up`，或直接 `node apps/scripts/dev-up.mjs`。
> 启动前需先启动 Docker Desktop（Redis 为硬依赖，不可达时脚本会直接报错退出）。

以下是逐个启动的手动步骤（排查用）：

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

## 许可证

本项目采用 **MIT License**，全文见 [`LICENSE`](LICENSE)。

- 你可自由使用、修改、分发、商用，只需保留版权与许可声明。
- 提交贡献前请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)（含贡献再许可条款）。

> **例外**：本地运行时依赖的决策模型 `StartLux-Decision-v1-4B-Q8_0-GGUF/` 为
> **CC BY-NC 4.0（非商用）**，未纳入本仓库版本控制（见 `.gitignore`）。若将本项目用于
> 商业用途，请自行替换为可商用模型或另行取得模型授权；本项目自身的 MIT 许可不改变该模型的许可限制。

