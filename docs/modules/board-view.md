# board-view（版图渲染与跨端契约）

- **位置**：`apps/web/src/view/`（纯 TS，禁 import express / node:fs / DOM）
- **形态**：`GameView`（server `/resources/game/{id}` 的 JSON）→ `BoardScene`（平台无关图元 IR）→ 宿主渲染
- **现状**：Web 端服务端直出 SVG + 浏览器 bundle 复用同一 scene；RN 端按本文件的映射表接 `react-native-svg`

## 分层

```
apps/web/src/view/
  types.ts        GameView / GameData / SgState（跨端共用数据契约）
  format.ts       escapeHtml / displayName / banknoteTotal
  text.ts         文本态渲染（纯函数，输出 HTML 片段）
  page.ts         双模式页面骨架（SSR + bootstrap + bundle）
  board/
    theme.ts      画布尺寸、六边形几何、6 人配色、字体、truncate
    layout 常量   位于 theme.ts（CASINO_RING_R / CASINO_CARD / PLAYER_PANEL）
    scene.ts      buildBoardScene(view) → BoardScene（分层 + 绝对坐标）
    svg.ts        BoardScene → SVG 字符串（Web 专用 adapter）
    manifest.ts   素材 id → 文件（由 `apps/scripts/board-assets.mjs` 生成）
apps/web/src/client/main.ts   浏览器 runtime（切换 / 轮询 / 激活刷新）
```

## BoardScene 契约（**跨端唯一接口**）

```ts
interface BoardScene {
  w: 760;                 // 逻辑宽（手机竖屏优先，等比缩放）
  h: number;              // 随玩家数/公共区行数变化
  layers: { name: string; nodes: SceneNode[] }[];
  assets: string[];       // 首屏需要预加载的 /board-assets/* URL
  meta: { round?; phase?; phaseLabel?; turnPlayerId?; seatCount };
}

type SceneNode =
  | { t: "rect";   x; y; w; h; rx?; fill; stroke?; sw?; opacity? }
  | { t: "circle"; cx; cy; r; fill; stroke?; sw?; opacity? }
  | { t: "path";   d; fill?; stroke?; sw?; opacity? }
  | { t: "text";   x; y; text; size; fill; anchor?; weight?; opacity? }
  | { t: "image";  x; y; w; h; rx?; href; opacity? };
```

**硬约束**（`apps/web/test-view.mts` 守卫）：

1. 只用上表 5 类图元，不得出现 `div`/`className`/`style`/`transform` 等平台属性；
2. 所有坐标与尺寸必须是有限数字（绝对坐标），不含百分比；
3. 文本节点长度受限（防溢出卡面）；
4. 素材 URL 只能以 `/board-assets/` 开头。

## RN 映射表（react-native-svg）

| SceneNode | react-native-svg |
|---|---|
| `rect` | `<Rect x y width height rx fill stroke strokeWidth opacity>` |
| `circle` | `<Circle cx cy r fill stroke strokeWidth opacity>` |
| `path` | `<Path d fill stroke strokeWidth>` |
| `text` | `<Text x y fontSize fill textAnchor fontWeight>` |
| `image` | `<Image x y width height href clipPath>`（`rx` 用 `clipPath` + `Rect`） |

```tsx
// 示例（RN）：一次取数，之后定时刷新
const [scene, setScene] = useState(() => buildBoardScene(view));
const S = { width: 760, height: scene.h };
<Svg viewBox={`0 0 ${S.width} ${S.height}`} width="100%" height={undefined}>
  {scene.layers.flatMap((l) => l.nodes.map(renderNode))}
</Svg>
```

- 数据来源与 Web 完全一致：`GET /tabletop_game/{gameId}/state?token=`（或 server `/resources/game/{id}`）。
- 文本态：`renderGameTextBody(view)` 返回 HTML，RN 需自行换行渲染（或用 WebView 兜底）。

## 版图布局（还原实物）

- 六边形骰盘：6 条边 = 6 个赌场（顺序 `CASINO_ANGLES = [-90,-30,30,90,150,210]`，对应 `casinos[0..5]`；与实物照黄色骰点数的对应关系待核定，见 `docs/games/las-vegas/props/board-layout.md`）。
- 赌场牌 220×150：编号徽标、店名、钞票背面（**只画张数，不画面额**）、骰子行（按数量降序，gray 骰单独配色，大骰用金框方块标记）。**小游戏不挂在赌场牌上**。
- 中央骰盘：轮次 / 阶段 / 当前玩家 / 本轮投掷。
- 玩家面板 2 列：座位色条、昵称（截断）、`我 / 机器人 / 行动中` 徽标、剩余骰、筹码、钞票张数与总额。
- **小游戏版面**（玩家面板下方）：整幅板块大图当版面（宽 720、高随宽高比），左上压 `A1 幸运之拳`（引擎名）、右上 `#N 赌场`，状态标记按印位压在图上的对应位置；多块按赌场号顺序平铺，不分页。
- **加密选项区块**：`renderSecretOptions(view)` 由文本态与版图态共用 —— 版图是默认视图，玩家不必切到文本态才能看到自己的代码；无选项时不渲染。
- 公共区：只列有内容的项（酒吧、摸彩、禁止、特权、掌声响起、灰骰组）。
- 底盘照片：`board-hex-*.webp`（760×760，裁六边形 + 掏空中央骰盘）。已启用（`USE_BOARD_TEXTURE = true`），以 `opacity 0.5` 压在卡牌下方当纹理（照片自带霓虹店名，全不透明会与卡牌叠字）。标定数据见 `apps/scripts/board-hex.mjs` 的 `CALIB`（按源文件名匹配；开口按木框内沿外扩 6% 吃掉 AI 残留黑边）。

## 素材清单与回退

- `manifest.ts` 的 `ASSETS` 由加工脚本重写；缺失 id → `resolveAsset()` 返回 `null` → 版图回退矢量占位，**绝不阻塞渲染**。
- 发布物在 `apps/web/public/board/`（git 忽略），由 `/board-assets/*` 提供，文件名带内容哈希 → `immutable` 长缓存。
- 加工脚本 `apps/scripts/board-assets.mjs`（inventory / build / preview）：边缘泛洪去背景 → 最大连通块 → 去斜 → 旋转 → 透明裁切 → WebP；单张 12–21KB。
- 映射表见 `docs/games/las-vegas/props/asset-mapping.md`（含 4 项待人工确认）。

## 小游戏锚点（`TILE_SLOTS`）

- 位置：`apps/web/src/view/board/scene.ts` 的 `TILE_SLOTS`（归一化 0–1，坐标以**紧裁后的板块图**为基准）。
- 已标定：`F2`（3 组 2/3/4 格）、`H1`（6 格）、`F1`（5 组）、`A2`（7 格钞票位）、`G1`（5 条摸彩）、`E2`（3 个吧位）、`E1`（禁止 + 箭头）、`C1` / `D2` / `G2` / `H2`（版心）。
- 微调方法：改对应数组里的 `[x, y]` → `node apps/scripts/board-assets.mjs build`（图片不变时无需重跑）→ 刷新页面。
- 标定参考图：`assets-src/derived/tile-anchors3.jpg`（每块板块带 0.1–0.9 网格）。

## RN 契约守卫

`apps/web/test-view-rn.mts`（已并入 `npm run test:web`）断言：

- 只用 `rect / circle / path / text / image` 五类原语，且每个原语**不含** RN 不支持的属性（`className`/`style`/`transform`…）；
- 所有数值属性为有限数字，文本 `anchor` 仅 `start|middle|end`；
- `path` 只允许绝对命令（RN `Path` 可直接消费）；
- 图元坐标不越出画布（RN 端等比缩放裁切不掉内容）；
- 同一份 `BoardScene` 能直接产出 SVG（Web 与 RN 共用构建逻辑，无二套业务代码）。

## 本地构建与自检

```bash
# 1) 素材（改图/改名后必跑）
node apps/scripts/board-assets.mjs build           # 生成 WebP + 重写 manifest.ts
node apps/scripts/board-assets.mjs preview         # 产物拼版目检

# 2) 前端（改了 view/ 或 client/ 后必跑）
npm --prefix apps/web run build:client             # esbuild → public/assets/app.js（≈20KB）

# 3) 自检
npm run test:web                                   # render / view / rn 三套纯函数测试
```

> `apps/web` 的开发进程用 `tsx`（无 watch）：改完代码要 `npm run dev:down && npm run dev:up:noqq` 重启。
> 页面与 `/state` 是 `no-store`，静态 bundle 是 `no-cache`（ETag 校验），改完刷新即可，无需清缓存。
