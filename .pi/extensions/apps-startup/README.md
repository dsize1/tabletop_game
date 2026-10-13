# apps-startup（Pi 扩展）

在 Pi TUI 中一键启动 / 停止本地服务链。

## 命令

- `/apps-startup start [--no-build] [--no-qq]` — 后台启动 `apps/scripts/dev-up.mjs`（server/web/agent/decision/MCP，默认含 qq-gateway）
- `/apps-startup stop [--keep-redis]` — 结束 dev-up 进程树 + 独立 qq-gateway + 残留服务；默认停 Redis 容器
- `/apps-startup status` — 列出 PID 文件与相关服务进程

## 实现要点

- `start`：`detached` 后台进程，stdout/stderr 落 `logs/dev-up.log` / `logs/dev-up.err.log`，PID 写 `data/run/dev-up.pid`；已在运行则跳过。
- `stop`：先按 PID 文件 `taskkill /F /T`，再兜底扫描（Windows：`Get-CimInstance` 匹配「node 命令行含项目根或 `dev-up.mjs`」或「本项目下 server/dev-task-mcp/game-engine-mcp/llama-server exe」），最后 `docker stop tabletop-game-redis`（可用 `--keep-redis` 保留）。
- 工厂函数不做任何进程/定时器副作用（仅注册命令），符合扩展生命周期约定。

## 手动等价操作

见 `docs/dev-environment.md` §6。
