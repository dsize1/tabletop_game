# ADR-0010：版图渲染、页内双模式与素材加工

- **状态**：已采纳（2026-10-18）
- **相关**：`docs/modules/web.md`、`docs/modules/board-view.md`、`docs/games/las-vegas/props/board-layout.md`、ADR-0003（公网可达）、ADR-0009（主动消息门禁，本决策不涉及）

## 背景

需求方提供 18 张实物照片（16 张小游戏板块 + 1 张六边形赌场盘 + 1 张封面），要求：

1. 游戏资源页默认展示**还原真实版图**的视觉态，同时**保留**原文本态；两者在同一页内切换（不改链接、不改 token 逻辑）。
2. 页面需具备**自动刷新**（现状：web 页纯服务端 HTML、无任何客户端 JS，轮询与重新激活刷新均缺失）。
3. 版图未来要被基于 RN 的 iOS/Android App 复用。
4. 图像模型只用于**离线加工已有素材**，不参与实时渲染。
5. 素材必须压缩（原图约 8MB），并尽量利用现有 Cloudflare Tunnel 做边缘缓存。

## 决策

### 1. 渲染位置：服务端生成场景 + SSR 首屏，浏览器复用同一套纯函数

- 新增 `apps/web/src/view/`（纯 TS，禁 `express` / `node:fs` / DOM）：`types / format / text / board/{theme,scene,svg,manifest} / page`。
- 服务端首屏直出 SVG + 文本 DOM；浏览器 bundle（esbuild，约 20KB）复用同一份渲染函数做轮询重渲染。
- **不引入 React/Vite**：现有站点是 Express + 字符串 HTML，引入框架会为「一页两态」付整套构建/水合成本。

### 2. 场景 IR（`BoardScene`）作为跨端唯一契约

- 只允许 5 类图元 `rect/circle/path/text/image`，**全部绝对坐标**，无平台属性（`test-view.mts` 守卫）。
- Web：`svg.ts` 机械映射为 SVG；RN：`react-native-svg` 同名组件 1:1 映射（见 `docs/modules/board-view.md`）。
- 复用边界放在**数据 + 场景**层，而不是「复用同一个页面」：RN 从 `/state` 取 JSON → `buildBoardScene` → 原生渲染。

### 3. 双模式与轮询

- 链接、token 校验、401/403/404/503 语义**保持不变**；新增 `GET /tabletop_game/{gameId}/state`（同一套鉴权）供轮询。
- 页内用 hash（`#board` / `#text`）切换，默认版图；`?view=text` 仅保留首屏直出文本态的能力（向后兼容）。
- 轮询：可见 10s、隐藏暂停、`visibilitychange`/`focus`/`pageshow`/`online` 立即刷新、失败指数退避至 60s。
- 页面与 `/state` 设 `Cache-Control: no-store`（token 在 query，绝不能进 CDN）。
- 无 JS → `<noscript>` 强制文本态。

### 4. 素材：离线加工 + 内容哈希 + 边缘缓存

- 原图（含水印）**不入库**，放本机 `assets-src/las/`（`assets-src/derived/`、`apps/web/public/board/` 均 git 忽略）。
- 加工流水线：透视校正 → 旋转 → 圆角矩形裁切 → ComfyUI + SDXL-inpainting 去手/去桌面/去水印 → 压缩为 WebP（首版 640×426 q75，单张 ≤35KB）→ 写 `manifest.ts`。
- 文字/编号区域禁止重绘；mask 外 SSIM ≥ 0.98；VLM 复读 `A1–H2` 标识校验。
- 发布物走 `/board-assets/*`（同域、内容哈希、`immutable`），Cloudflare Cache Rule 做边缘缓存；未命中素材回退矢量占位。

## 取舍

| 方案 | 结论 |
|---|---|
| React/Vue SPA | 否（成本 ≫ 收益；现网无构建链） |
| Canvas 2D 渲染 | 否（SVG 天然可缩放、可 SSR、可增量 DOM 替换、RN 映射更直接） |
| 新增 npm workspace 包 | 暂否（避免动依赖树；`view/` 已按可抽包方式组织，RN 开工再抽） |
| 客户端直连 Rust server | 否（保持 BFF：浏览器只连 web，token 由 web 校验） |
| 图像生成参与实时渲染 | 否（延迟/随机性不可控，素材必须预生成且可缓存） |

## 影响

- `apps/web` 增加客户端 bundle 构建步骤（`build:client`），`dev`/`build`/`test` 脚本已同步。
- 静态素材目录与构建产物入 `.gitignore`；加工脚本产出 `manifest.ts` 需随发布一并更新。
- 后续接入 RN 时：复用 `view/types.ts` + `view/board/scene.ts`，无需改服务端。
