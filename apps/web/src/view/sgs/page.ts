// 三国杀页面骨架与样式（P3.1）：左右分栏 + 公共区/个人区上下 + 三档断点。
//
// 约束（`docs/games/sanguosha/ui-spec.md` §2/§3/§8）：
// - Chromium ≥ 88 / iOS Safari ≥ 14；禁用 `:has()`、容器查询、`dvh`；
// - 布局用 flex + `clamp()` + 百分比定位，不依赖 JS 测量；
// - 特殊区/详情统一居中浮层（P3.2 起接入），本文件只留 `#sgs-overlay` 容器。

import type { GameView } from "../types.js";
import { escapeHtml } from "../format.js";
import { renderPersonalArea } from "./personal.js";
import { renderPublicArea, renderSidePanel } from "./public.js";
import { renderSgsTextBody } from "./text.js";
import type { SgsViewData } from "./types.js";

export interface SgsPageOpts {
  gameId: string;
  token: string;
  openid: string;
  roomId: string;
  pollMs?: number;
}

/** JSON 内联到 <script> 时必须转义 `<`。 */
function inlineJson(value: unknown): string {
  return JSON.stringify(value).replace(/</g, "\\u003c");
}

export const SGS_STYLES = `
:root { color-scheme: dark; --sgs-bg:#0b0f14; --sgs-panel:#161b22; --sgs-line:#30363d; --sgs-gold:#e3b341; --sgs-text:#e6edf3; --sgs-muted:#8b949e; }
* { box-sizing: border-box; }
html, body { margin:0; padding:0; background:var(--sgs-bg); color:var(--sgs-text);
  font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif;
  -webkit-text-size-adjust:100%; }
body { overflow-x:hidden; }
button { font: inherit; color: inherit; }
.sgs-topbar { display:flex; gap:10px; align-items:center; padding:8px 12px 6px;
  position:sticky; top:0; z-index:6; background:linear-gradient(#0b0f14 75%, rgba(11,15,20,.86)); }
.sgs-topbar h1 { font-size:16px; margin:0; letter-spacing:.5px; }
.sgs-topbar .meta { font-size:12px; color:var(--sgs-muted); }
.sgs-topbar .grow { margin-left:auto; }
.sgs-layout { display:flex; gap:10px; padding:0 10px 84px; align-items:stretch; min-height:calc(100vh - 46px); }
.sgs-side { align-self:flex-start; flex:0 0 clamp(200px,22vw,320px); background:var(--sgs-panel); border:1px solid var(--sgs-line);
  border-radius:12px; padding:10px 12px; display:flex; flex-direction:column; gap:10px;
  position:sticky; top:44px; max-height:calc(100vh - 60px); }
.sgs-side h3 { font-size:12px; margin:0 0 6px; color:var(--sgs-muted); font-weight:600; letter-spacing:.5px; }
.sgs-side .grow { flex:1; min-height:0; display:flex; flex-direction:column; }
.sgs-phases { display:flex; flex-wrap:wrap; gap:4px; }
.sgs-phase { font-size:11px; padding:2px 6px; border-radius:999px; background:#21262d; color:var(--sgs-muted); }
.sgs-phase.on { background:var(--sgs-gold); color:#1a1206; font-weight:700; }
.sgs-history { list-style:none; margin:0; padding:0; overflow:auto; font-size:12px; line-height:1.5; }
.sgs-history li { padding:2px 0; border-bottom:1px dashed #21262d; }
.sgs-log-actor { color:#9fd7a0; font-weight:600; }
.sgs-main { flex:1 1 auto; min-width:0; display:flex; flex-direction:column; gap:10px; }
.sgs-public { position:relative; flex:1 1 auto; min-height:clamp(260px,44vh,640px);
  background:radial-gradient(circle at 50% 45%, #131a23 0%, #0d1219 70%); border:1px solid var(--sgs-line);
  border-radius:14px; overflow:hidden; container-type:normal; }
.sgs-seats { position:absolute; inset:0; }
.sgs-opponent { position:absolute; width:clamp(84px,15%,160px); transform:translateX(-50%);
  background:rgba(22,27,34,.92); border:1px solid var(--sgs-line); border-radius:10px; padding:6px 7px;
  font-size:11px; line-height:1.35; }
.sgs-opponent.anchor-right { transform:translateX(0); }
.sgs-opponent.anchor-left { transform:translateX(-100%); }
.sgs-opponent.anchor-center { transform:translateX(-50%); }
.sgs-opponent.current { border-color:var(--sgs-gold); box-shadow:0 0 0 1px rgba(227,179,65,.4); }
.sgs-opponent.dead { opacity:.55; filter:grayscale(.6); }
.sgs-opponent-head { display:flex; gap:4px; align-items:center; font-weight:600; }
.sgs-opponent .sgs-general { display:flex; gap:4px; align-items:center; color:#c9d1d9; }
.sgs-flip { font-size:10px; padding:0 4px; border-radius:6px; background:#4a2a5a; color:#e7c7ff; }
.sgs-marks { font-size:11px; color:#c9b27a; display:flex; flex-wrap:wrap; gap:4px; margin:2px 0; }
.sgs-mini-row.marks { color:#c9b27a; }
.sgs-opponent .sgs-kingdom { font-size:10px; padding:0 4px; border:1px solid var(--sgs-line); border-radius:6px; }
.sgs-identity { font-size:10px; padding:0 4px; border-radius:6px; background:#21262d; color:#c9d1d9; }
.sgs-identity.lord { background:#7a5b12; color:#ffe9b0; }
.sgs-identity.rebel { background:#5a1f1f; color:#ffb3b3; }
.sgs-identity.spy { background:#23324a; color:#b9d2ff; }
.sgs-hp { display:flex; align-items:center; gap:3px; margin:2px 0; }
.sgs-hp .hp { width:7px; height:7px; border-radius:50%; display:inline-block; background:#3a2020; }
.sgs-hp .hp.on { background:#e5484d; }
.sgs-hp-num { font-size:10px; color:var(--sgs-muted); margin-left:2px; }
.sgs-hand-mini { display:flex; align-items:center; gap:2px; flex-wrap:wrap; }
/* 他人手牌背面堆叠：每张仅露左侧一小截（负 margin），第一张不缩 */
.sgs-hand-mini .sgs-card.back + .sgs-card.back { margin-left:-18px; }
.sgs-hand-mini .sgs-card.back { box-shadow:1px 1px 2px rgba(0,0,0,.45); }
.sgs-hand-count { font-size:11px; color:#c9d1d9; }
.sgs-mini-row { display:flex; flex-wrap:wrap; gap:3px; margin-top:2px; }
.sgs-mini-row.judge .sgs-chip { background:#2a2033; }
.sgs-dead-mark { position:absolute; inset:auto 6px 4px auto; color:#ff7b72; font-weight:700; }
.sgs-center { position:absolute; left:50%; top:44%; transform:translate(-50%,-50%); text-align:center; width:min(46%, 360px); }
.sgs-round { font-size:12px; color:#c9d1d9; margin-bottom:6px; }
.sgs-deck-row { display:flex; gap:12px; justify-content:center; }
.sgs-deck { background:var(--sgs-panel); border:1px solid var(--sgs-line); border-radius:10px; padding:6px 10px; font-size:12px; min-width:86px; }
.sgs-deck-label { display:block; color:var(--sgs-muted); font-size:10px; }
.sgs-deck-discard { font-size:11px; color:#c9d1d9; margin-top:2px; }
.sgs-waiting { margin-top:8px; font-size:12px; color:var(--sgs-gold); }
.sgs-winner { margin-top:8px; font-size:14px; font-weight:700; color:var(--sgs-gold); }
.sgs-zone-list { margin-top:8px; font-size:11px; color:#c9d1d9; display:flex; flex-direction:column; gap:2px; }
.sgs-personal { flex:0 0 auto; background:var(--sgs-panel); border:1px solid var(--sgs-line); border-radius:14px;
  padding:10px 12px; display:flex; flex-direction:column; gap:8px; }
.sgs-me { display:flex; align-items:center; gap:8px; flex-wrap:wrap; }
.sgs-me-head { display:flex; gap:6px; align-items:center; }
.sgs-skills { display:flex; gap:6px; flex-wrap:wrap; margin-left:auto; }
.sgs-skill { font-size:12px; padding:4px 10px; border-radius:999px; border:1px solid #3d4757; background:#1c232c; cursor:pointer; }
.sgs-skill.disabled { opacity:.45; cursor:default; }
.sgs-me-row { display:flex; gap:10px; flex-wrap:wrap; font-size:12px; }
.sgs-equips, .sgs-judge { display:flex; gap:6px; align-items:center; flex-wrap:wrap; }
.sgs-equip.active { border-color:var(--sgs-gold); box-shadow:0 0 0 1px rgba(227,179,65,.45) inset; }
.sgs-equip { display:flex; gap:4px; align-items:center; background:#1c232c; border:1px solid #2b3644;
  border-radius:8px; padding:3px 6px; font-size:11px; }
.sgs-equip-label { color:var(--sgs-muted); font-size:10px; }
.sgs-hand-head { display:flex; align-items:center; gap:6px; font-size:11px; color:#c9d1d9; }
.sgs-hand { display:flex; gap:6px; overflow-x:auto; padding:2px 2px 6px; scrollbar-width:thin;
  overscroll-behavior:contain; -webkit-overflow-scrolling:touch; min-height:76px; }
.sgs-card { flex:0 0 auto; width:clamp(48px, 7vw, 64px); height:clamp(66px, 9.4vw, 88px);
  border-radius:8px; background:#f5f2e8; color:#1a1a1a; border:1px solid #cfc8b6;
  display:flex; flex-direction:column; align-items:center; justify-content:center; gap:2px;
  font-size:12px; line-height:1.1; user-select:none; touch-action:manipulation; }
.sgs-card.red { color:#b3261e; }
.sgs-card.back { width:26px; height:34px; background:linear-gradient(135deg,#2a3a56,#1b2740); border-color:#38507a; }
.sgs-card-rank { font-weight:700; font-size:13px; }
.sgs-card-name { font-size:11px; text-align:center; padding:0 2px; }
.sgs-card.selected { outline:2px solid var(--sgs-gold); transform:translateY(-4px); }
.sgs-chip { font-size:10px; padding:1px 5px; border-radius:6px; background:#21262d; border:1px solid #2b3644; white-space:nowrap; }
.sgs-chip.red { color:#ff9b95; }
.sgs-pending { border:1px dashed #3d4757; border-radius:10px; padding:6px 8px; font-size:12px; color:#c9d1d9; }
.sgs-pending-head { color:var(--sgs-gold); margin-bottom:4px; }
.sgs-pending-actions { display:flex; flex-wrap:wrap; gap:6px; }
.btn.small { padding:6px 10px; min-height:34px; font-size:12px; border-radius:8px; }
.sgs-card.playable { border-color:#8a6d1f; box-shadow:0 0 0 1px rgba(227,179,65,.35) inset; }
.sgs-gx { display:flex; gap:10px; justify-content:center; }
.sgs-gx-col-head { font-size:11px; color:var(--sgs-muted); text-align:center; margin-bottom:4px; }
.sgs-gx-face { display:flex; flex-direction:column; align-items:center; line-height:1.1; }
.sgs-gx-face.red { color:#ff9b95; }
.sgs-gx-rank { font-weight:700; font-size:12px; }
.sgs-gx-name { font-size:11px; }
.sgs-gx-moves { display:flex; gap:2px; }
.sgs-gx-move { width:20px; height:18px; padding:0; font-size:9px; border-radius:4px; border:1px solid #3d4757;
  background:#1c232c; color:#c9d1d9; cursor:pointer; }
.sgs-gx-slot.dragging { z-index:5; box-shadow:0 8px 24px rgba(0,0,0,.6); border-color:var(--sgs-gold); position:relative; }
.sgs-gx-col { display:flex; flex-direction:column; gap:4px; }
.sgs-gx-slot { width:clamp(60px,12vw,110px); height:clamp(34px,5vw,44px); border-radius:8px;
  border:1px dashed #3d4757; display:flex; align-items:center; justify-content:center; gap:4px; font-size:11px; }
.sgs-gx-slot.empty { color:var(--sgs-muted); }
.sgs-gx-id { color:var(--sgs-muted); font-size:10px; }
.sgs-gx-card { font-weight:600; }
.sgs-wugu { border:1px solid #3d4757; border-radius:10px; padding:6px 8px; font-size:12px; }
.sgs-wugu-head { color:var(--sgs-gold); margin-bottom:4px; }
.sgs-wugu-cards { display:flex; gap:6px; flex-wrap:wrap; }
.sgs-wugu-card { padding:4px 8px; border-radius:8px; background:#1c232c; border:1px solid #3d4757; cursor:pointer; }
.sgs-ops { display:flex; gap:8px; align-items:center; flex-wrap:wrap; }
.sgs-ops .sgs-selected { margin-right:auto; font-size:12px; }
.btn { padding:8px 14px; border-radius:10px; border:1px solid #3d4757; background:var(--sgs-gold); color:#1a1206;
  font-weight:700; cursor:pointer; min-height:40px; }
.btn.ghost { background:#1c232c; color:#c9d1d9; }
.btn:disabled { opacity:.45; cursor:default; }
.sgs-statusbar { position:fixed; left:0; right:0; bottom:0; z-index:6; display:flex; gap:10px;
  font-size:11px; color:var(--sgs-muted); padding:4px 12px calc(4px + env(safe-area-inset-bottom));
  background:linear-gradient(rgba(11,15,20,0), #0b0f14 45%); }
.sgs-statusbar .grow { margin-left:auto; }
.sgs-statusbar .warn { color:#f0883e; }
#sgs-msg { color:#9fd7a0; max-width:52vw; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
#sgs-msg.warn { color:#f0883e; }
body.busy { cursor:progress; }
.sgs-overlay { position:fixed; inset:0; z-index:100; display:none; }
.sgs-overlay.on { display:block; }
.sgs-overlay-mask { position:absolute; inset:0; background:rgba(0,0,0,.55); }
.sgs-overlay-card { position:relative; margin:6vh auto 0; background:var(--sgs-panel); border:1px solid var(--sgs-line);
  border-radius:14px; max-width:min(92vw,720px); max-height:min(80vh,640px); overflow:auto; padding:14px 16px;
  overscroll-behavior:contain; }
.sgs-overlay-card.wide { max-width:min(96vw,900px); }
.sgs-overlay-head { display:flex; align-items:center; gap:8px; margin-bottom:10px; }
.sgs-overlay-head h2 { font-size:15px; margin:0; }
.sgs-overlay-close { margin-left:auto; width:34px; height:34px; border-radius:999px; border:1px solid #3d4757;
  background:#1c232c; color:#c9d1d9; font-size:20px; line-height:1; cursor:pointer; }
.sgs-overlay-body { font-size:13px; line-height:1.6; }
.sgs-overlay-lock { margin-left:auto; font-size:11px; padding:3px 8px; border-radius:999px;
  background:#3a2c0e; color:#f0c765; border:1px solid #7a5b12; }
.sgs-zone-note p { margin:0 0 6px; }
.sgs-zone-actions { display:flex; flex-wrap:wrap; gap:6px; margin:8px 0; }
.sgs-zone-group { border-top:1px solid #262d38; padding-top:8px; margin-top:8px; }
.sgs-zone-group h3 { font-size:12px; color:var(--sgs-muted); margin:0 0 4px; }
.sgs-multi-cards { display:flex; flex-wrap:wrap; gap:6px; margin:8px 0; }
.sgs-multi-card { padding:8px 10px; border-radius:8px; border:1px solid #3d4757; background:#1c232c; color:#c9d1d9; cursor:pointer; }
.sgs-multi-card.on { border-color:var(--sgs-gold); background:#2a2413; color:#ffe9b0; font-weight:700; }
.sgs-overlay.blocking .sgs-overlay-mask { cursor:default; }
.sgs-detail { border-collapse:collapse; width:100%; }
.sgs-detail th, .sgs-detail td { border-bottom:1px solid #262d38; padding:6px 8px; text-align:left; vertical-align:top; }
.sgs-detail th { width:6.5em; color:var(--sgs-muted); font-weight:600; }
.sgs-big { font-size:20px; font-weight:700; }
.sgs-big.red { color:#ff9b95; }
.sgs-discard-list { margin:6px 0 0; padding-left:22px; max-height:min(56vh,460px); overflow:auto; }
.sgs-discard-list li { padding:1px 0; }
.sgs-deck-btn { cursor:pointer; text-align:left; color:inherit; }
.sgs-card.pressed, .sgs-opponent.pressed, .sgs-chip.pressed, .sgs-deck-btn.pressed { outline:2px solid var(--sgs-gold); }
.sgs-text { display:none; padding:12px 16px 90px; }
.sgs-text h2 { font-size:16px; } .sgs-text h3 { font-size:13px; color:var(--sgs-muted); margin:14px 0 4px; }
.sgs-text ul { padding-left:18px; margin:0; } .sgs-text li { font-size:13px; line-height:1.6; }
.sgs-form { display:flex; gap:8px; align-items:center; flex-wrap:wrap; margin-top:10px; }
.sgs-form input { flex:1 1 220px; padding:8px; border-radius:8px; border:1px solid #3d4757; background:#0f141b; color:var(--sgs-text); }
.sgs-form button { padding:8px 14px; border-radius:8px; border:0; background:var(--sgs-gold); color:#1a1206; font-weight:700; }
.sgs-drawer-toggle { display:none; }
.muted { color:var(--sgs-muted); }
@media (max-width: 1023px) {
  .sgs-drawer-toggle { display:inline-flex; align-items:center; gap:6px; font-size:12px; color:#c9d1d9;
    background:#1c232c; border:1px solid #3d4757; border-radius:999px; padding:4px 10px; cursor:pointer; }
  .sgs-side { display:none; position:fixed; left:8px; top:44px; bottom:56px; z-index:8; width:min(78vw,320px);
    box-shadow:0 10px 30px rgba(0,0,0,.5); max-height:none; }
  #sgs-drawer:checked ~ .sgs-layout .sgs-side { display:flex; }
  .sgs-layout { padding:0 8px 84px; }
}
@media (max-width: 639px) {
  .sgs-public { min-height:300px; }
  .sgs-opponent { width:88px; font-size:10px; padding:5px; }
  .sgs-center { width:52%; }
  .sgs-ops { position:sticky; bottom:26px; background:var(--sgs-panel); border-top:1px solid var(--sgs-line);
    padding-top:8px; }
}
@media (min-width: 1024px) { .sgs-hand { flex-wrap:wrap; overflow:visible; } }
/* 横屏手机/矮视口：压缩公共区与个人区，操作条贴底 */
@media (max-height: 520px) {
  .sgs-layout { min-height:0; padding-bottom:64px; gap:8px; }
  .sgs-public { min-height:180px; }
  .sgs-personal { padding:6px 10px; gap:5px; }
  .sgs-hand { min-height:58px; padding-bottom:2px; }
  .sgs-card { width:44px; height:60px; }
  .sgs-ops { position:fixed; left:0; right:0; bottom:0; z-index:7; background:var(--sgs-panel);
    border-top:1px solid var(--sgs-line); padding:8px 12px calc(8px + env(safe-area-inset-bottom)); }
  .sgs-side { max-height:calc(100vh - 56px); }
}
@media (prefers-reduced-motion: reduce) { .sgs-card { transition:none; } }
`;

/** 页面主体（SSR 与客户端重渲染共用）。`actions` = 本人合法动作（服务端按权限过滤）。 */
export function renderSgsApp(
  data: SgsViewData,
  names?: Record<string, string>,
  actions?: Array<{ id: string; description: string }>,
): string {
  const mySeat = data.player_area?.seat ?? data.public_board.turn.seat;
  return (
    `<div class="sgs-main">` +
    renderPublicArea(data, mySeat, names) +
    renderPersonalArea(data.player_area, data.public_board, data.special_zones, names, actions) +
    `</div>`
  );
}

/** 完整页面（SSR）。 */
export function renderSgsPage(view: GameView, opts: SgsPageOpts): string {
  const data = (view.player?.data ?? view.public.data) as unknown as SgsViewData;
  const names = view.names;
  const board = data.public_board;
  const title = `三国杀 · 第 ${board.round} 轮`;
  const bootstrap = {
    gameId: opts.gameId,
    token: opts.token,
    roomId: opts.roomId,
    openid: opts.openid,
    pollMs: opts.pollMs ?? 10000,
  };
  const actions = view.actions ?? [];
  const textBody = renderSgsTextBody(data, names, {
    gameId: opts.gameId,
    token: opts.token,
    roomId: opts.roomId,
  });
  return (
    `<!doctype html><html lang="zh-CN"><head>` +
    `<meta charset="utf-8">` +
    `<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">` +
    `<meta name="robots" content="noindex,nofollow">` +
    `<title>${escapeHtml(title)}</title>` +
    `<style>${SGS_STYLES}</style>` +
    `</head><body>` +
    `<div class="sgs-topbar"><h1>三国杀</h1>` +
    `<span class="meta">第 ${board.round} 轮 · ${escapeHtml(board.turn.player_id ?? "-")}</span>` +
    `<label class="sgs-drawer-toggle" for="sgs-drawer">辅助区</label>` +
    `<span class="grow"></span><span class="meta" id="sgs-sync">—</span></div>` +
    `<input type="checkbox" id="sgs-drawer" hidden>` +
    `<div class="sgs-layout">` +
    renderSidePanel(data, names) +
    `<div class="sgs-main" id="sgs-app">` +
    renderPublicArea(data, data.player_area?.seat ?? board.turn.seat, names) +
    renderPersonalArea(data.player_area, board, data.special_zones, names, actions) +
    `</div>` +
    `</div>` +
    `<div class="sgs-statusbar"><span>token 仅存于本页链接</span>` +
    `<span class="grow"></span><span id="sgs-msg"></span><span id="sgs-warn"></span>` +
    `<span id="sgs-updated">—</span></div>` +
    `<div class="sgs-overlay" id="sgs-overlay" role="dialog" aria-modal="true" aria-label="特殊区"></div>` +
    `<div class="sgs-text" id="sgs-text">${textBody}</div>` +
    `<noscript><style>#sgs-app,.sgs-side,.sgs-statusbar{display:none!important}.sgs-text{display:block!important}</style></noscript>` +
    `<script id="sgs-bootstrap" type="application/json">${inlineJson(bootstrap)}</script>` +
    `<script type="module" src="/assets/sgs.js"></script>` +
    `</body></html>`
  );
}
