// markdown(GFM) → HTML 渲染。
//
// 本期在 web 端渲染（先 markdownToHtml）；后续 game/dev 引擎将以 MCP 能力
// 暴露统一的「web 渲染原语」，届时此模块迁移为调用统一原语。

import { marked } from "marked";

/** 文档树节点（path 为相对所属根的 `/` 分隔路径）。 */
export interface DocsTreeNode {
  name: string;
  type: "dir" | "file";
  path: string;
  md?: boolean;
  children?: DocsTreeNode[];
}

/** 一个文档根的子树。 */
export interface DocsRootTree {
  name: string;
  path: string;
  children: DocsTreeNode[];
}

const ESCAPE_MAP: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#39;",
};

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ESCAPE_MAP[c] ?? c);
}

/** 渲染 markdown（GFM：表格 / 复选框 / 代码块）。 */
export function renderMarkdown(md: string): string {
  return marked.parse(md, { gfm: true, breaks: false, async: false }) as string;
}

function encodeQuery(s: string): string {
  return encodeURIComponent(s);
}

function renderNodes(nodes: DocsTreeNode[], rootName: string, token: string): string {
  const parts: string[] = [];
  for (const node of nodes) {
    if (node.type === "dir") {
      const inner = node.children?.length ? renderNodes(node.children, rootName, token) : "";
      parts.push(
        `<details class="dir"><summary>${escapeHtml(node.name)}</summary>` +
          `<div class="children">${inner}</div></details>`,
      );
    } else if (node.md) {
      const href =
        `/dev_docs/file?token=${encodeQuery(token)}` +
        `&root=${encodeQuery(rootName)}&path=${encodeQuery(node.path)}`;
      parts.push(`<div class="file"><a href="${href}">${escapeHtml(node.name)}</a></div>`);
    } else {
      // 仅 .md 可导航预览；其它类型只展示名称
      parts.push(`<div class="file muted">${escapeHtml(node.name)}</div>`);
    }
  }
  return parts.join("");
}

/** 文件树 body HTML：原生 <details> 折叠，.md 可点、其它仅显示名称。 */
export function renderDocsTree(roots: DocsRootTree[], token: string): string {
  if (!roots.length) return `<p class="error">暂无可用文档根</p>`;
  return roots
    .map(
      (r) =>
        `<details class="root" open><summary>${escapeHtml(r.name)}</summary>` +
        `<div class="children">${renderNodes(r.children ?? [], r.name, token)}</div></details>`,
    )
    .join("");
}

/** 包裹为独立 HTML 页（移动端优先）。 */
export function renderPage(title: string, bodyHtml: string): string {
  return `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)}</title>
<style>
  :root { color-scheme: light dark; }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
      "Microsoft YaHei", sans-serif;
    line-height: 1.6;
    background: #f7f8fa;
    color: #24292f;
  }
  .container { max-width: 860px; margin: 0 auto; padding: 20px 16px 48px; }
  .markdown-body {
    background: #fff;
    border: 1px solid #e5e7eb;
    border-radius: 10px;
    padding: 20px 24px;
    overflow-wrap: break-word;
  }
  .markdown-body h1, .markdown-body h2, .markdown-body h3 {
    border-bottom: 1px solid #e5e7eb; padding-bottom: .3em; margin-top: 1.2em;
  }
  .markdown-body h1 { font-size: 1.6em; }
  .markdown-body h2 { font-size: 1.35em; }
  .markdown-body h3 { font-size: 1.15em; }
  .markdown-body pre {
    background: #f6f8fa; padding: 12px; border-radius: 8px; overflow-x: auto;
  }
  .markdown-body code {
    background: #f6f8fa; padding: .15em .4em; border-radius: 4px;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: .9em;
  }
  .markdown-body pre code { background: none; padding: 0; }
  .markdown-body table { border-collapse: collapse; width: 100%; margin: 1em 0; }
  .markdown-body th, .markdown-body td {
    border: 1px solid #d0d7de; padding: 6px 12px; text-align: left;
  }
  .markdown-body th { background: #f6f8fa; }
  .markdown-body blockquote {
    margin: 1em 0; padding: .1em 1em; color: #57606a;
    border-left: 4px solid #d0d7de; background: #f6f8fa;
  }
  .markdown-body input[type="checkbox"] { margin-right: 6px; }
  .tree {
    background: #fff; border: 1px solid #e5e7eb; border-radius: 10px;
    padding: 12px 18px;
  }
  .tree details { margin: 2px 0; }
  .tree summary { cursor: pointer; padding: 2px 0; font-weight: 600; }
  .tree .children { margin-left: 18px; }
  .tree .file { padding: 2px 0; }
  .tree .file a { color: #0969da; text-decoration: none; }
  .tree .file a:hover { text-decoration: underline; }
  .tree .file.muted { color: #8b949e; }
  .breadcrumb { margin-bottom: 12px; font-size: .9em; color: #57606a; }
  .breadcrumb a { color: #0969da; text-decoration: none; }
  .breadcrumb a:hover { text-decoration: underline; }
  .error { text-align: center; padding: 60px 20px; color: #57606a; }
  .error h1 { font-size: 4em; margin: 0; color: #cf222e; }
  .muted { color: #8b949e; font-size: .9em; }
  .sg-cell {
    display: inline-block; min-width: 1.6em; margin: 2px 4px 2px 0;
    padding: 0 4px; border: 1px solid #d0d7de; border-radius: 4px;
    text-align: center; font-size: .95em;
  }
  .sg-cell.empty { color: #afb8c1; }
  .sg-group { display: inline-block; margin-right: 14px; }
  .sg-group.full { opacity: .55; }
  .sg-group b { font-family: ui-monospace, Consolas, monospace; }
  @media (prefers-color-scheme: dark) {
    body { background: #0d1117; color: #c9d1d9; }
    .markdown-body { background: #161b22; border-color: #30363d; }
    .markdown-body h1, .markdown-body h2, .markdown-body h3 { border-color: #30363d; }
    .markdown-body pre, .markdown-body code { background: #0d1117; }
    .markdown-body th { background: #21262d; }
    .markdown-body th, .markdown-body td { border-color: #30363d; }
    .markdown-body blockquote { color: #8b949e; border-color: #30363d; background: #161b22; }
    .tree { background: #161b22; border-color: #30363d; }
    .tree .file a, .breadcrumb a { color: #58a6ff; }
    .tree .file.muted, .breadcrumb { color: #8b949e; }
    .sg-cell { border-color: #30363d; }
    .sg-cell.empty { color: #484f58; }
  }
</style>
</head>
<body>
<main class="container">
<div class="markdown-body">
${bodyHtml}
</div>
</main>
</body>
</html>`;
}

/** 游戏资源视图（server `/resources/game/{gameId}` 返回）。 */
export interface GameView {
  game_id: string;
  room_id: string;
  openid: string;
  public: { status?: string; players?: string[]; data?: GameData };
  player: { status?: string; players?: string[]; data?: GameData };
  /** openid → 群昵称（server 从 Redis `names:{group_id}` 读取）。 */
  names?: Record<string, string>;
}

interface GameData {
  phase?: string;
  round?: number;
  turn?: number;
  start_player?: string;
  seats?: Array<{
    player_id: string;
    small_remaining?: number;
    big_remaining?: number;
    chips?: number;
    banknotes?: number[];
  }>;
  casinos?: Array<{
    dice?: Record<string, number>;
    /** 各玩家在该赌场的大骰颗数（`dice` 已把大骰计为 2 点权重）。 */
    big_dice?: Record<string, number>;
    banknotes?: number[];
    small_games?: string[];
  }>;
  winners?: string[];
  scores?: Record<string, number>;
  /** 小游戏子状态（引擎 `sg` 字段；缺失时降级显示）。 */
  sg?: {
    f2_slots?: Array<number | null> | null;
    h1_slots?: Array<{ owner: string; dice: number; big?: number } | null> | null;
    gray_groups?: number[];
    /** E2 酒吧：玩家 → {small,big}（大骰也算 1 颗）。 */
    bars?: Record<string, { small?: number; big?: number }>;
    game_field?: Record<string, number>;
    /** H2 金框：{owner, small, big}（大骰也算 1 颗）。 */
    gold_frame?: { owner: string; small?: number; big?: number } | null;
    banned?: number | null;
    arrow?: number;
    lottery?: number[];
    indicator?: number;
    /** C1：本轮「先到 5 颗」的玩家（拿到即得 $10 万）。 */
    c1_winner?: string | null;
    privilege?: string | null;
    /** 只在**个人视图**出现：加密选项（A1/G1）代码 → 真实含义。 */
    pending_options?: {
      game?: string;
      stage?: string;
      multi?: boolean;
      options?: Array<{ code: string; count?: number; reward?: string; value?: number; label?: string }>;
    } | null;
  };
}

const SG_TITLES: Record<string, string> = {
  A1: "幸运之拳",
  A2: "大奖挑战",
  B1: "主场时间",
  B2: "50% 的机会",
  C1: "掌声响起",
  C2: "厄运降临",
  D1: "发薪愉快",
  D2: "权力优势",
  E1: "禁止进入",
  E2: "醉倒了吗",
  F1: "阻止他",
  F2: "危机就是转机",
  G1: "黑色摸彩箱",
  G2: "双倍颁奖",
  H1: "好骰难求",
  H2: "我的抉择",
};

/** 玩家 id → 展示名（缺失时用 id）。 */
function who(names: Record<string, string> | undefined, id: string): string {
  return escapeHtml(names?.[id] ?? id);
}

function cell(text: string, filled: boolean): string {
  return `<span class="sg-cell${filled ? "" : " empty"}">${text}</span>`;
}

/** 单个小游戏的状态摘要（F2 九格 / H1 六格 / 游戏场 / 金框 / 酒吧 等）。 */
function sgStateHtml(id: string, dat: GameData, names?: Record<string, string>): string {
  const sg = dat.sg ?? {};
  const casinos = Array.isArray(dat.casinos) ? dat.casinos : [];
  switch (id) {
    case "F2": {
      // 3 组 9 格：a=1–2（2 格）、b=3–5（3 格）、c=6–9（4 格）。
      const groups = [
        { key: "a", start: 0, cap: 2, reward: "1 筹码" },
        { key: "b", start: 2, cap: 3, reward: "$3 万" },
        { key: "c", start: 5, cap: 4, reward: "放入/收回" },
      ];
      const slots = Array.isArray(sg.f2_slots) ? sg.f2_slots : [];
      const cells = groups
        .map((g) => {
          let filled = 0;
          const boxes = Array.from({ length: g.cap }, (_, k) => {
            const v = slots[g.start + k];
            if (v === null || v === undefined) return cell("□", false);
            filled += 1;
            return cell(`●${Number(v)}`, true);
          }).join("");
          const full = filled >= g.cap;
          return (
            `<span class="sg-group${full ? " full" : ""}">` +
            `<b>${g.key}</b> ${g.reward}（${filled}/${g.cap}${full ? " 已满" : ""}）<br>${boxes}</span>`
          );
        })
        .join("");
      const grayOnBoard = casinos
        .map((c, i) => ({ i: i + 1, n: Number(c.dice?.gray ?? 0) }))
        .filter((x) => x.n > 0)
        .map((x) => `${x.i} 号×${x.n}`)
        .join("、");
      return (
        cells +
        `<div class="muted">场上灰骰：${escapeHtml(grayOnBoard || "无")}｜` +
        `格子数字 = 灰骰来源赌场号</div>`
      );
    }
    case "H1": {
      const slots = Array.isArray(sg.h1_slots) ? sg.h1_slots : [];
      if (!slots.length) return '<span class="muted">6 格未初始化</span>';
      return slots
        .map((s, i) => {
          if (!s) return cell(`${i + 1}`, false);
          const big = Number(s.big ?? 0);
          return cell(`${i + 1}:${who(names, s.owner)}×${s.dice}${big ? "(大)" : ""}`, true);
        })
        .join("");
    }
    case "G2": {
      const field = sg.game_field ?? {};
      const items = Object.entries(field)
        .filter(([, n]) => Number(n) > 0)
        .map(([p, n]) => `${who(names, p)}×${n}`);
      return items.length ? escapeHtml(items.join("、")) : '<span class="muted">游戏场空</span>';
    }
    case "H2": {
      const gf = sg.gold_frame;
      if (!gf) return '<span class="muted">金框空</span>';
      const small = Number(gf.small ?? 0);
      const big = Number(gf.big ?? 0);
      const detail = [small ? `小×${small}` : "", big ? `大×${big}` : ""].filter(Boolean).join(" ");
      return `金框：${who(names, gf.owner)}${detail ? `（${detail}）` : ""}`;
    }
    case "E2": {
      const bars = sg.bars ?? {};
      const items = Object.entries(bars).map(([p, d]) => {
        const small = Number(d?.small ?? 0);
        const big = Number(d?.big ?? 0);
        const detail = [small ? `小×${small}` : "", big ? `大×${big}` : ""]
          .filter(Boolean)
          .join(" ");
        return `${who(names, p)}：${detail || "0"}`;
      });
      return items.length ? escapeHtml(items.join("、")) : '<span class="muted">酒吧空</span>';
    }
    case "F1": {
      const groups = Array.isArray(sg.gray_groups) ? sg.gray_groups : [];
      if (!groups.length) return '<span class="muted">已全部放入赌场</span>';
      return escapeHtml(groups.map((n, i) => `第 ${i + 1} 组×${n}`).join("、"));
    }
    case "G1": {
      const lot = Array.isArray(sg.lottery) ? sg.lottery : [];
      const shown = lot.map((n) => (Number(n) > 0 ? `$${Number(n)} 万` : "?"));
      return escapeHtml(`${lot.length} 个摸彩标记：${shown.join("、")}`);
    }
    case "A2":
      return `指示物：第 ${Number(sg.indicator ?? 0) + 1} 格`;
    case "E1": {
      const banned = sg.banned;
      const arrow = Number(sg.arrow ?? 0) + 1;
      return escapeHtml(
        `${banned === null || banned === undefined ? "未禁止" : `禁止 ${Number(banned) + 1} 号赌场`}｜箭头第 ${arrow} 格`,
      );
    }
    case "D2": {
      const p = sg.privilege;
      return p ? `特权：${who(names, p)}` : '<span class="muted">特权标记在游戏场</span>';
    }
    case "C1": {
      const w = sg.c1_winner;
      return w
        ? `本轮已由 ${who(names, w)} 抢先（$10 万，每轮仅一次）`
        : '<span class="muted">本轮未触发（先到 5 颗得 $10 万）</span>';
    }
    default:
      return '<span class="muted">—</span>';
  }
}

/** 渲染拉斯维加斯牌面页（公共视图 + 个人资源）。数据缺失时安全降级。 */
export function renderGamePage(view: GameView): string {
  const d = view.public?.data ?? {};
  // 个人视图（含本人加密选项）单独取，避免误用公共视图。
  const mine = (view.player?.data ?? {}) as GameData;
  const sgMine = mine.sg ?? {};
  const seats = Array.isArray(d.seats) ? d.seats : [];
  const casinos = Array.isArray(d.casinos) ? d.casinos : [];
  const cur = seats[d.turn ?? -1]?.player_id;

  const casinoRows = casinos
    .map((c, i) => {
      const dice = c.dice ?? {};
      const big = c.big_dice ?? {};
      const total = Object.values(dice).reduce((a, b) => a + Number(b), 0);
      const detail = Object.entries(dice)
        .map(([p, n]) => {
          const b = Number(big[p] ?? 0);
          // `dice` 是等效骰数（大骰算 2），单独标出其中大骰颗数，避免误读成 2 颗小骰。
          return `${escapeHtml(view.names?.[p] ?? p)}×${n}${b > 0 ? `（含大骰 ${b}）` : ""}`;
        })
        .join("、");
      const notes = Array.isArray(c.banknotes) ? c.banknotes.length : 0;
      const sgs = Array.isArray(c.small_games) ? c.small_games.join("/") : "";
      return (
        `<tr><td>${i + 1}</td><td>${total}` +
        `${detail ? `<div class="muted">${detail}</div>` : ""}</td>` +
        `<td>${notes} 张</td><td>${escapeHtml(sgs)}</td></tr>`
      );
    })
    .join("");

  const seatRows = seats
    .map((s) => {
      const notes = Array.isArray(s.banknotes) ? s.banknotes : [];
      const sum = notes.reduce((a, b) => a + b, 0);
      const isMe = s.player_id === view.openid;
      const isTurn = s.player_id === cur;
      const display = view.names?.[s.player_id] ?? String(s.player_id);
      return (
        `<tr${isMe ? ' style="font-weight:600"' : ""}>` +
        `<td>${escapeHtml(display)}${isMe ? "（我）" : ""}${isTurn ? " ⏳" : ""}</td>` +
        `<td>${s.small_remaining ?? "-"} 小 / ${s.big_remaining ?? "-"} 大</td>` +
        `<td>${s.chips ?? 0}</td>` +
        `<td>${notes.length} 张（${sum / 10000} 万）</td></tr>`
      );
    })
    .join("");

  const finished = d.phase === "finished" || view.public?.status === "finished";
  const result = finished
    ? `<h3>终局</h3><p>胜者：${escapeHtml((d.winners ?? []).join("、"))}</p>`
    : "";

  // 小游戏状态：把掛在各赌场的板块逐个展开（F2 九格、H1 六格、游戏场/金框/酒吧……）。
  const sgRows = casinos
    .flatMap((c, i) =>
      (Array.isArray(c.small_games) ? c.small_games : []).map((id) => {
        const title = SG_TITLES[id] ? ` ${SG_TITLES[id]}` : "";
        return `<tr><td>${i + 1}</td><td>${escapeHtml(id + title)}</td><td>${sgStateHtml(
          id,
          d,
          view.names,
        )}</td></tr>`;
      }),
    )
    .join("");
  // 加密选项：只在本人页出现（群里只发代码）。
  const opts = sgMine.pending_options;
  const codes = opts?.options ?? [];
  const multi = Boolean(opts?.multi);
  const secretBox = codes.length
    ? `<h3>🔐 你的加密选项${opts?.game ? `（${escapeHtml(opts.game)}）` : ""}</h3>` +
      `<table><thead><tr><th>代码</th><th>含义</th>${multi ? "" : "<th>奖励</th>"}</tr></thead><tbody>` +
      codes
        .map((o) => {
          const meaning =
            o.count !== undefined ? `藏 ${o.count} 颗标记` : escapeHtml(o.label ?? "${" + String(o.value ?? 0) + "} 万");
          const reward = o.reward !== undefined ? `<td>${escapeHtml(o.reward)}</td>` : "";
          return `<tr><td><b>${escapeHtml(o.code)}</b></td><td>${meaning}</td>${reward}</tr>`;
        })
        .join("") +
      `</tbody></table>` +
      `<blockquote>${
        multi
          ? `把想放进 <b>A 堆</b> 的代码发到群里（<b>可多选</b>，逗号分隔），如：<code>#选 ${escapeHtml(
              codes[0]?.code ?? "代码",
            )}，${escapeHtml(codes[1]?.code ?? "代码")}</code>；未选的进 B 堆。`
          : `把想用的<b>代码</b>发到群里即可（如：<code>#选 ${escapeHtml(codes[0]?.code ?? "代码")}</code>）。`
      }代码 5 分钟内有效；过期或链接失效时，在群里发「#恢复」重新获取。</blockquote>`
    : "";

  const smallGames = sgRows
    ? `<h3>小游戏状态</h3>` +
      `<table><thead><tr><th>赌场</th><th>小游戏</th><th>状态</th></tr></thead>` +
      `<tbody>${sgRows}</tbody></table>`
    : "";

  const body = `
    <h2>🎲 拉斯维加斯 · 第 ${d.round ?? "-"} 轮</h2>
    <p class="muted">阶段 ${escapeHtml(String(d.phase ?? view.public?.status ?? "-"))}｜当前 ${
      escapeHtml(cur ? (view.names?.[cur] ?? cur) : "-")
    }</p>
    <h3>赌场</h3>
    <table><thead><tr><th>#</th><th>骰子</th><th>钞票</th><th>小游戏</th></tr></thead>
    <tbody>${casinoRows}</tbody></table>
    <h3>玩家</h3>
    <table><thead><tr><th>玩家</th><th>手中骰</th><th>筹码</th><th>已得钞票</th></tr></thead>
    <tbody>${seatRows}</tbody></table>
    ${secretBox}
    ${smallGames}
    ${result}
  `;
  return renderPage("拉斯维加斯 · 牌面", body);
}

/** 统一错误页（401/403/404/410）。 */
export function renderErrorPage(status: number, message: string): string {
  return renderPage(
    `${status}`,
    `<div class="error"><h1>${status}</h1><p>${escapeHtml(message)}</p></div>`,
  );
}
