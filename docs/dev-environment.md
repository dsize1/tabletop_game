# 本地开发环境

> Docker / Redis / 镜像加速等环境配置，供本地跑通链路使用。

## 1. Docker Desktop

- 安装：`winget install -e --id Docker.DockerDesktop`（需 UAC 确认）
- 后端：使用 WSL2（Settings → General → 勾选 **Use the WSL 2 based engine**；Settings → Resources → WSL Integration → 勾选 Ubuntu）
- 就绪标志：托盘图标显示「Docker Desktop is running」

## 2. 镜像加速（国内）

直连 Docker Hub（`registry-1.docker.io`）常超时。配置 registry 镜像，编辑 `~/.docker/daemon.json`：

```json
{
  "registry-mirrors": [
    "https://docker.m.daocloud.io",
    "https://docker.1panel.live"
  ]
}
```

改完**重启 Docker Desktop**（托盘 → Restart）生效。

急于拉取时，可临时用镜像源 + 重打 tag（无需重启引擎）：

```bash
docker pull docker.m.daocloud.io/library/redis:7-alpine
docker tag docker.m.daocloud.io/library/redis:7-alpine redis:7-alpine
```

## 3. Redis 起停

```bash
docker compose up -d        # 启动（后台）
docker compose ps           # 查看状态（应为 running）
docker compose down         # 停止并删除容器（数据卷保留）
docker compose down -v      # 停止并连数据一起删除
```

验证：

```bash
docker exec tabletop-game-redis redis-cli ping                     # PONG
docker exec tabletop-game-redis redis-cli XRANGE qq.raw - +        # 查入站事件流
```

## 4. 常见问题

| 现象 | 处理 |
|---|---|
| `docker: command not found` | 重开终端（PATH 未刷新）；或把 DockerDesktop `resources\bin` 加入 PATH |
| 拉镜像超时（registry-1.docker.io） | 配镜像加速（见 §2） |
| `docker-credential-desktop: not found` | Docker 的 bin 目录不在 PATH，补 PATH 后重试 |
| 端口 6379 被占用 | `docker compose down` 后重来，或改 `docker-compose.yml` 端口 |

## 5. Node / Rust

- Node >= 18：`cd apps && npm install`
- Rust：rustup stable-msvc，`cd crates && cargo check`；crates.io 镜像用 rsproxy.cn（见初始化记录）

## 6. 启动顺序（跑通消息链路）

### 一键启动（推荐）

```bash
npm run dev:up         # Redis + cargo build + server + decision + web + agent-gateway
npm run dev:up:qq      # 追加 qq-gateway（需已配 QQBOT_APP_ID/QQBOT_APP_SECRET）
```

`apps/scripts/dev-up.mjs` 前台运行，Ctrl+C 停止全部子进程（含 MCP/llama-server 进程树）。
可选参数：`--no-build`（跳过 cargo build）、`--no-decision`（不启动 4.5G 决策模型）。

### 手动逐个启动（排查用）

依赖：Redis 已起（§3）；Rust 二进制已编译（`cd crates && cargo build`）。

| 顺序 | 进程 | 命令 | 职责 |
|---|---|---|---|
| 1 | server | `cd crates && cargo run -p server` | bot-adapter（qq.raw→internal）+ 健康自检 + /health |
| 2 | agent-gateway | `cd apps/agent-gateway && npm run dev` | 消费 internal → 决策 → 聊天/猜拳 → qq.push；spawn game-engine-mcp |
| 3 | qq-gateway | `cd apps/qq-gateway && npm run dev` | QQ 入站（→qq.raw）+ 出站（qq.push→发群消息） |

> `game-engine-mcp` 由 agent-gateway 通过 MCP stdio 自动 spawn，无需手动启动；但需先 `cargo build` 编译出 `crates/target/debug/game-engine-mcp(.exe)`。

### 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis 地址（server / agent-gateway / qq-gateway 共用） |
| `SQLITE_PATH` | `data/tabletop.db` | SQLite 文件路径（server / game-engine-mcp 共用） |
| `SERVER_PORT` | `3000` | server HTTP 端口 |
| `DEV_WHITELIST` | 空 | 开发白名单 member_openid，逗号分隔（agent-gateway） |
| `GAME_ENGINE_MCP` | `crates/target/debug/game-engine-mcp(.exe)` | game-engine-mcp 二进制路径（agent-gateway） |
| `QQBOT_APP_ID` / `QQBOT_APP_SECRET` | 空 | QQ 机器人凭证（qq-gateway） |

### 健康检查

```bash
curl http://127.0.0.1:3000/health   # server 聚合状态（读缓存，10s 周期自检）
node apps/scripts/status.mjs        # 探针脚本（等同 /health）
```
