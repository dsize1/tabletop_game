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
npm run dev:up         # Redis + cargo build + server + decision + web + agent-gateway + qq-gateway（默认含 QQ）
npm run dev:up:noqq    # 不启动 qq-gateway
npm run dev:up:qq      # 兼容旧命令，等价于 dev:up
```

> 以上命令在**仓库根目录**执行（仓库根 `package.json` 仅做转发，真正的 npm workspaces 根在 `apps/`）。
> 也可等价地用 `cd apps && npm run dev:up`，或直接 `node apps/scripts/dev-up.mjs`。

`apps/scripts/dev-up.mjs` 前台运行，Ctrl+C 停止全部子进程（含 MCP/llama-server 进程树）。
可选参数：`--no-qq`（不启动 qq-gateway）、`--no-build`（跳过 cargo build）、`--no-decision`（不启动 4.5G 决策模型）。

> **Redis 是硬依赖**：脚本先探活 127.0.0.1:6379，不可达时自动尝试 `docker compose up -d`；若仍不可达则**直接报错退出（exit 1）**，不再启动后续服务（否则 server 会因连接被拒秒退，导致「已启动」后整条链路连带消失）。请先启动 Docker Desktop 并按 §1/§3 确认 Redis。
> qq-gateway 需已配 `apps/qq-gateway/.env` 的 `QQBOT_APP_ID`/`QQBOT_APP_SECRET`；凭证缺失时脚本会打印告警并跳过，不影响其余服务。
> 不带 qq-gateway 跑会导致「QQ 群收不到 / 发不出消息」的静默故障——排查时先看启动横幅里的 `已启动：... / qq`。

### Pi TUI 一键起停（`/apps-startup`）

在 Pi TUI 中直接起停整条服务链（扩展：`.pi/extensions/apps-startup/`）：

| 命令 | 作用 |
|---|---|
| `/apps-startup start [--no-build] [--no-qq]` | 后台（detached）启动 `dev-up`，PID 写入 `data/run/dev-up.pid`，日志 `logs/dev-up.log` |
| `/apps-startup stop [--keep-redis]` | 结束 dev-up 进程树 + 独立 qq-gateway + 残留服务；默认连 Redis 容器一起停 |
| `/apps-startup status` | 列出 dev-up PID 文件与相关服务进程 |

- `start` 默认会 `cargo build`；加 `--no-build` 跳过以加快启动。已在运行则不重复启动。
- `stop` 默认停 Redis 容器（数据卷保留）；加 `--keep-redis` 只停应用。进程匹配按「命令行含项目根 / `dev-up.mjs`」+ 本项目下服务 exe，避免误杀同名进程。
- 与 `npm run dev:up` 等价，只是改为后台运行 + 可一键停止；关机前建议先执行 `/apps-startup stop`。
- 命令行等价物 `npm run dev:down [--keep-redis]`（`apps/scripts/dev-down.mjs`，按 `data/run/dev-up.pid` 结束整棵进程树）。
- ⚠️ **不要**直接 `taskkill` 单个子进程（如 `server.exe`）：dev-up 视子进程意外退出为致命，会连带停掉 web/agent/qq/decision。停服一律走 `/apps-startup stop` 或 `npm run dev:down`。

### 手动逐个启动（排查用）

依赖：Redis 已起（§3）；Rust 二进制已编译（`cd crates && cargo build`）。

| 顺序 | 进程 | 命令 | 职责 |
|---|---|---|---|
| 1 | server | `cd crates && cargo run -p server` | bot-adapter（qq.raw→internal）+ 健康自检 + /health |
| 2 | agent-gateway | `cd apps/agent-gateway && npm run dev` | 消费 internal → 决策 → 聊天/猜拳 → qq.push；spawn game-engine-mcp |
| 3 | qq-gateway | `cd apps/qq-gateway && npm run dev` | QQ 入站（→qq.raw）+ 出站（qq.push→发群消息） |

> `game-engine-mcp` / `dev-task-mcp` / `docs-mcp` 均由 agent-gateway 通过 MCP stdio 自动 spawn，无需手动启动；但需先 `cargo build` 编译出 `crates/target/debug/*-mcp(.exe)`。

### 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis 地址（server / agent-gateway / qq-gateway 共用） |
| `SQLITE_PATH` | `data/tabletop.db` | SQLite 文件路径（server / game-engine-mcp 共用） |
| `SERVER_PORT` | `3000` | server HTTP 端口 |
| `DEV_WHITELIST` | 空 | 开发白名单 member_openid，逗号分隔（agent-gateway） |
| `GAME_ENGINE_MCP` | `crates/target/debug/game-engine-mcp(.exe)` | game-engine-mcp 二进制路径（agent-gateway） |
| `DOCS_MCP` | `crates/target/debug/docs-mcp(.exe)` | docs-mcp 二进制路径（agent-gateway） |
| `DOCS_CONFIG` | `config/dev-docs.json` | 开发文档预览配置（roots + exclude；docs-mcp / server 共读） |
| `PUBLIC_BASE_URL` | 空 | web 公网基址（短链拼接；agent-gateway） |
| `QQBOT_APP_ID` / `QQBOT_APP_SECRET` | 空 | QQ 机器人凭证（qq-gateway） |

### 健康检查

```bash
curl http://127.0.0.1:3000/health   # server 聚合状态（读缓存，10s 周期自检）
node apps/scripts/status.mjs        # 探针脚本（等同 /health）
```

> 也可在仓库根执行 `npm run status`。Redis 不可达时探针会打印可读提示并以 exit 1 结束（而非 ioredis 未处理异常堆栈）。
