// 双模式页面骨架（版图 / 文本，页内切换，不跳转）。
//
// 设计要点：
// - 链接与鉴权不变；token 只随页面与 `/state` 请求传递，不写入 localStorage；
// - 首屏 SSR 同时输出「版图 SVG」与「文本 DOM」，靠 `data-view` 切换，避免白屏；
// - 无 JS 时由 `<noscript>` 强制显示文本态（版图仍需脚本切换/轮询）；
// - 轮询与切换由 `/assets/app.js` 接管（见 `src/client/main.ts`）。

import type { GameView } from "./types.js";
import { renderGameTextBody } from "./text.js";
import { buildBoardScene, phaseLabel } from "./board/scene.js";
import { renderBoardSvg, renderPreloadLinks } from "./board/svg.js";
import { escapeHtml } from "./format.js";

/** JSON 内联到 <script> 时必须转义 `<`，避免提前闭合标签。 */
function inlineJson(value: unknown): string {
  return JSON.stringify(value).replace(/</g, "\\u003c");
}

const STYLES = `
:root { color-scheme: dark; }
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; overflow-x: hidden; }
body {
  background: #0b0f14; color: #e6edf3;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif;
  -webkit-text-size-adjust: 100%;
}
.wrap { max-width: 1040px; margin: 0 auto; padding: 10px 10px 40px; overflow-x: hidden; }
.topbar {
  display: flex; align-items: center; gap: 10px;
  padding: 6px 4px 10px; position: sticky; top: 0; z-index: 5;
  background: linear-gradient(#0b0f14 70%, rgba(11,15,20,0.85)); backdrop-filter: blur(6px);
}
.topbar h1 { font-size: 17px; margin: 0; font-weight: 700; letter-spacing: .5px; }
.topbar .round { font-size: 13px; color: #98a5b3; }
.seg { margin-left: auto; display: flex; background: #171e27; border: 1px solid #2b3644; border-radius: 999px; padding: 2px; }
.seg button {
  appearance: none; border: 0; background: transparent; color: #98a5b3;
  font-size: 13px; padding: 5px 12px; border-radius: 999px; cursor: pointer;
}
.seg button[aria-selected="true"] { background: #e3b341; color: #1a1206; font-weight: 700; }
.statusbar { display: flex; gap: 10px; font-size: 12px; color: #6e7b8a; padding: 0 4px 8px; }
.statusbar .grow { margin-left: auto; }
.statusbar .warn { color: #f0883e; }
#board-view { display: none; }
#text-view { display: none; }
main[data-view="board"] #board-view { display: block; }
main[data-view="text"] #text-view { display: block; }
.board-svg { width: 100%; max-width: 100%; height: auto; display: block; border-radius: 14px; background: #0b0f14; }
.panel { background: #161b22; border: 1px solid #30363d; border-radius: 12px; padding: 14px 16px; }
.panel h2 { font-size: 1.25em; margin: .2em 0 .5em; }
.panel h3 { font-size: 1.05em; margin: 1em 0 .4em; }
.panel table { border-collapse: collapse; width: 100%; margin: .4em 0 1em; font-size: 14px; }
.panel th, .panel td { border: 1px solid #30363d; padding: 6px 8px; text-align: left; vertical-align: top; }
.panel th { background: #21262d; }
.panel .muted { color: #8b949e; font-size: .9em; }
.panel blockquote {
  margin: .8em 0; padding: .1em 1em; color: #8b949e;
  border-left: 4px solid #30363d; background: #0d1117;
}
.panel code { background: #0d1117; padding: .1em .35em; border-radius: 4px; font-family: ui-monospace, Consolas, monospace; }
.sg-cell { display: inline-block; min-width: 1.6em; margin: 2px 4px 2px 0; padding: 0 4px; border: 1px solid #30363d; border-radius: 4px; text-align: center; }
.sg-cell.empty { color: #484f58; }
.sg-group { display: inline-block; margin-right: 14px; }
.sg-group.full { opacity: .55; }
.empty-state { color: #6e7b8a; text-align: center; padding: 40px 12px; }
@media (max-width: 560px) {
  .panel table { font-size: 13px; }
  .panel th, .panel td { padding: 4px 6px; }
}
`;

export interface AppPageOpts {
  /** 游戏 id（用于 /state 轮询）。 */
  gameId: string;
  /** 当前短链 token（原样透传给 /state）。 */
  token: string;
  /** 初始模式（首屏 SSR 决定，默认 board）。 */
  mode?: "board" | "text";
}

/** 渲染双模式游戏页（SSR 首屏 + bootstrap JSON + 客户端 bundle）。 */
export function renderAppPage(view: GameView, opts: AppPageOpts): string {
  const mode = opts.mode === "text" ? "text" : "board";
  const scene = buildBoardScene(view);
  const d = view.public?.data ?? {};
  const modeLabel = mode === "text" ? "文本" : "版图";

  const bootstrap = inlineJson({
    gameId: opts.gameId,
    token: opts.token,
    mode,
    pollMs: 10_000,
  });

  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="dark">
<title>拉斯维加斯 · ${escapeHtml(modeLabel)}</title>
${renderPreloadLinks(scene)}
<style>${STYLES}</style>
<noscript><style>#board-view{display:none}#text-view{display:block}
.seg{display:none}</style></noscript>
</head>
<body>
<div class="wrap">
  <header class="topbar">
    <h1>🎲 拉斯维加斯</h1>
    <span class="round">第 ${escapeHtml(String(d.round ?? "-"))} 轮 · ${escapeHtml(phaseLabel(d.phase))}</span>
    <div class="seg" role="tablist" aria-label="显示模式">
      <button type="button" role="tab" data-mode="board" aria-selected="${mode === "board"}">版图</button>
      <button type="button" role="tab" data-mode="text" aria-selected="${mode === "text"}">文本</button>
    </div>
  </header>
  <div class="statusbar">
    <span id="conn">已连接</span>
    <span class="grow" id="updated">—</span>
  </div>
  <main id="app" data-view="${mode}">
    <section id="board-view" aria-label="版图">${renderBoardSvg(scene)}</section>
    <section id="text-view" class="panel" aria-label="文本牌面">${renderGameTextBody(view)}</section>
  </main>
</div>
<script type="application/json" id="bootstrap">${bootstrap}</script>
<script type="module" src="/assets/app.js"></script>
</body>
</html>`;
}
