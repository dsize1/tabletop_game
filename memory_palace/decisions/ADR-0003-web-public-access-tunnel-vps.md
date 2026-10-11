# ADR-0003：web 公网可达 —— Cloudflare Tunnel 首选 + VPS 内网穿透兜底

- 状态：已采纳（Accepted）
- 日期：2026-10-11

## 背景

web（Express）是独立资源展示站点，玩家需通过手机/浏览器访问
`https://.../tabletop_game/{gameId}/player/{qqid}?token=xxx`。
项目无公网 IP、无固定公网地址。QQ 消息链路走 WebSocket 主动出站，无需公网 IP、不受影响；但 web 页必须公网可达，否则外网玩家无法访问。

## 决策

1. **首选 Cloudflare Tunnel**：web 监听 `127.0.0.1:PORT`，cloudflared 加密隧道反代到自有域名（托管 Cloudflare DNS），零公网 IP、自动 HTTPS、隐藏源站、断线自愈。
2. **兜底 VPS 内网穿透**：Tunnel 大陆跨境访问不佳/不可达时，用一台 VPS 做公网入口——本地反连 VPS（frp / ssh -R），VPS 侧 Nginx 反代 + HTTPS 对外。
3. **应用层鉴权不变**：短期 token + server auth 照旧，穿透层只解决「可达」，不承担鉴权。

## 理由

- Cloudflare Tunnel：零成本、零公网 IP、自动 HTTPS、隐藏源站、断线自愈；劣势是大陆跨境访问稳定性不可控，免费版无 SLA。
- VPS 内网穿透：可控性强（节点可自选、可备案、可加白名单），但需一台 VPS + 维护成本；作为 Tunnel 效果不佳时的降级路径，而非默认项。
- 备选：国内内网穿透 SaaS（受供应商限制）、自建 frp（本质等同 VPS 兜底）、云函数/Serverless 托管（改动大、暂不需要）。

## 影响

- 部署侧新增 cloudflared（首选）/ frp（兜底）组件，均只影响 web 出口，不触碰 QQ 消息链路。
- 需要一个自有域名（Tunnel）与一台 VPS（兜底时）。
- web 需处理反代头：`trust proxy`、`CF-Connecting-IP` / `X-Forwarded-For` 取真实 IP。
- 长期对公众运营需评估备案/合规；先以 Tunnel 内测/小圈子落地。
- 详见 `docs/modules/web.md`。
