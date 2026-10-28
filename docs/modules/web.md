# web（资源展示）

- **语言**：Node.js / TypeScript + Express
- **位置**：`apps/web`

## 职责

- 独立站点（BFF）：服务端调 `server` 鉴权 + 拉取资源，浏览器不直连 server / SQLite
- 路由：
  - `GET /dev_task/{sessionId}?token=xxx`：dev-task 计划文档页（校验 token → 拉 markdown → GFM 渲染 HTML）
  - `GET /dev_docs?token=xxx`：开发文档预览树页（校验 token → server 拉树 JSON → `<details>` 折叠；仅 `.md` 可点）
  - `GET /dev_docs/file?token=xxx&root=<name>&path=<rel>`：文档内容页（仅 `.md`，marked 渲染）
  - `GET /tabletop_game/{gameId}/player/{openid}?token=xxx`：游戏资源页（**双模式**：默认**版图**，页内 `#board` / `#text` 切换；服务端 SSR 首屏 + 客户端 bundle，见 `docs/modules/board-view.md`）
  - `GET /tabletop_game/{gameId}/state?token=xxx`：**状态 JSON**（页面轮询用；同一套 token 校验，401/404/503 语义同页面）
  - `POST /tabletop_game/{gameId}/action`：**动作提交**（P3）。`fetch` 用 JSON（`Accept: application/json`）直接返回 `{ok,message}`；无 JS 时普通表单 `application/x-www-form-urlencoded` 提交后 `303` 回页面（PRG）。token 资源范围必须等于 `game:{room_id}`（越权 403），服务端转发到 `server /internal/game/{gameId}/{roomId}/action` 执行并落库。
  - `GET /board-assets/*`：版图素材（公开静态，文件名带内容哈希 → `immutable`）；`GET /assets/*`：页面 runtime bundle
  - `GET /health`：健康检查（cloudflared 探活）
- markdown(GFM) 渲染：`src/render.ts`（marked，表格/复选框/代码块）。**后续** game/dev 引擎以 MCP 能力暴露统一的「web 渲染原语」，届时此层迁移为调用统一原语

### 页面运行时（2026-10-18 起）

- **双模式**：`?view=text` 仅用于首屏直出文本态（向后兼容）；正常链接不带参数，默认版图。切换用 hash（`#board` / `#text`）+ `history.replaceState`，**不跳转、不丢 token**。
- **轮询**：可见时 10s 拉 `/state`；`document.hidden` 暂停；`visibilitychange` / `focus` / `pageshow(bfcache)` / `online` 立即刷新；失败指数退避 10→20→40→60s，页脚显示「连接中断」与「更新于 HH:MM:SS」。
- **降级**：禁用 JS 时 `<noscript>` 强制显示文本态（SSR 已含完整文本）。
- **缓存**：页面与 `/state` 一律 `Cache-Control: no-store`；`/board-assets` `immutable` 一年，可被 Cloudflare 边缘缓存（Cache Rule：`path startswith /board-assets/`）。
- 渲染函数在 `src/view/`（纯函数，服务端与 bundle 共用），页面骨架在 `src/view/page.ts`，`src/render.ts` 只保留文档预览/错误页/文本态独立页。

### 移动端形态（2026-10-19，ADR-0011）

> 结论：**P0 不建原生 App（先 PWA）；P1 需要 App 图标/启动器时用 Tauri 2 壳；RN/Expo 延后**。取舍与触发条件见 ADR-0011。

> **2026-10-27 修订（三国杀阶段）**：端侧壳（Tauri/RN）**整体延后**；三国杀页面先在**手机浏览器 + PC 浏览器**验收，验收通过后再决定壳形态。页面基线定为 **Chromium ≥ 88 / iOS Safari ≥ 14**（禁 `:has()`、容器查询、`dvh`），布局用 flex + `clamp()` autosize + 百分比定位，不依赖原生桥。详见 `docs/games/sanguosha/ui-spec.md`。

- **P0（PWA）**：移动端交付物 = 现有页 + manifest + 图标 + `display: standalone`；**不改鉴权、不改路由、不改渲染**。
- **P1（App 壳）**：Tauri 2，Android 先行；壳加载线上域名（`PUBLIC_BASE_URL` 同源页），**不启用 IPC / Rust command** → 「改 web 即发布」，零发版更新。iOS 需 macOS 构建（阻塞项，见 ADR-0011 影响）。
- **对 web 的壳化约束**（现状已满足，后续改动不得破坏）：
  1. 页面在「无 IPC 的裸 WebView」中自洽：token 只走 URL query + `/state` 请求，**不依赖原生桥、不落 localStorage**（`src/client/main.ts`）；
  2. 页面与 `/state` 保持 `no-store`；壳不做离线缓存，避免 token 落盘；
  3. 不把核心功能押在浏览器专有能力（Service Worker / Notification 等）上；PWA 只用 manifest + 图标；
  4. 需兼容老旧系统 WebView（国产 ROM 无 Play 长期不更新）→ 上线前评估 `build:client` 的 target 由 `es2020` 降到 `es2017`（**本期未改**）。
- **未做（待立项）**：PWA manifest/图标、Tauri 工程、Android 工具链（JDK/SDK/NDK）、iOS 需求与上架意愿确认。

### 统一「web 渲染原语」评估（2026-10-13）

- docs 预览本期已按「MCP 原语（`docs-mcp`：授权 + 范围校验）→ server 资源 API → web 渲染」分层落地。
- 现状：game 资源页已接入——**脱敏在 server 侧**（读 `game_state` 快照 → **按 `game_id` 分派引擎** `public_state`/`player_view`；P3 起含 `sanguosha`，未注册引擎不再原样下发内部状态），web 只渲染 JSON（`src/view/text.ts`：赌场、玩家、**加密选项**、小游戏状态四张表；`src/view/board/scene.ts`：版图场景）；加密选项来自 `player_view`，公共视图不含。token 资源范围为 `game:{room_id}`，room_id 由资源范围解析。后续如需多引擎统一渲染，再评估下沉为引擎侧「web 渲染原语」。
- 纯函数渲染有单测：`apps/web/test-render.mts`（`npm --workspace @tabletop-game/web run test`）。

## 约定

- 经 `server` 的 auth HTTP API 校验短期 token（`POST /auth/verify`）
- 资源数据经 `server` 的资源查询 API 获取（不直连 SQLite / 文件系统）：`GET /resources/dev_task/{sessionId}/plan`、`GET /resources/docs/tree`、`GET /resources/docs/file`
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
