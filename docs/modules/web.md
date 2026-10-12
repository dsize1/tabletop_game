# web（资源展示）

- **语言**：Node.js / TypeScript + Express
- **位置**：`apps/web`

## 职责

- 独立站点（BFF）：服务端调 `server` 鉴权 + 拉取资源，浏览器不直连 server / SQLite
- 路由：
  - `GET /dev_task/{sessionId}?token=xxx`：dev-task 计划文档页（校验 token → 拉 markdown → GFM 渲染 HTML）
  - `GET /tabletop_game/{gameId}/player/{openid}?token=xxx`：游戏资源页（引擎未实现，占位「暂未开放」）
  - `GET /health`：健康检查（cloudflared 探活）
- markdown(GFM) 渲染：`src/render.ts`（marked，表格/复选框/代码块）。**后续** game/dev 引擎以 MCP 能力暴露统一的「web 渲染原语」，届时此层迁移为调用统一原语

## 约定

- 经 `server` 的 auth HTTP API 校验短期 token（`POST /auth/verify`）
- 资源数据经 `server` 的资源查询 API 获取（不直连 SQLite）：`GET /resources/dev_task/{sessionId}/plan`
- token 绑定 openid（member_openid）+ 资源范围；越权 403、无效/过期 401、不存在 404、服务器不可用 503
- `trust proxy`：经 cloudflared 反代后源站只见回环，真实 IP 取 `CF-Connecting-IP` / `X-Forwarded-For`

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `WEB_PORT` | `18080` | 监听端口（cloudflared ingress 反代到该端口） |
| `SERVER_URL` | `http://127.0.0.1:3000` | server 内部 HTTP 地址 |

## 部署与公网可达

> QQ 消息链路走 WebSocket **主动出站**，无需公网 IP、不受影响、保持不变。

web 是独立站点，玩家通过手机/浏览器访问，**必须公网可达**。无公网 IP 时，外网玩家无法访问。

| 层级 | 方案 | 说明 |
|---|---|---|
| 首选 | **Cloudflare Tunnel** | web 监听 `127.0.0.1:PORT`，cloudflared 加密隧道反代到自有域名（托管 Cloudflare DNS）。零公网 IP、自动 HTTPS、隐藏源站。 |
| 兜底 | **VPS 内网穿透** | Tunnel 跨境访问不佳/不可达时，用一台 VPS 做公网入口：本地反连 VPS（frp / ssh -R），VPS 侧 Nginx 反代 + HTTPS 对外。 |

### 两方案通用约束

- **鉴权不变**：短期 token + server auth 照旧；穿透层只解决「可达」，不替代应用层鉴权（可选 Cloudflare Access 叠加，勿与 token 机制重复）。
- **真实 IP**：经反代后源站见回环地址，取真实 IP 需处理 `CF-Connecting-IP` / `X-Forwarded-For`，Express 设 `trust proxy`。
- **备案/合规**：长期对公众稳定运营需评估 ICP 备案；先以 Tunnel 内测/小圈子落地，跑顺后再评估。
- **延迟**：web 页为低频资源展示，回合制桌游对延迟不敏感；Tunnel 主要风险是大陆跨境「稳定性/可达性」而非延迟。
