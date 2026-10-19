// 版图场景构建（纯函数）：GameView → BoardScene。
//
// BoardScene 是「平台无关图元 IR」：Web 端用 `svg.ts` 渲染成 SVG 字符串；
// RN 端用 react-native-svg 做 1:1 映射（rect/circle/path/text/image）。
// 约束：只用这 5 类图元 + 绝对数字坐标，不含任何 DOM/平台属性（有守卫测试）。

import { displayName, banknoteTotal } from "../format.js";
import type { CasinoData, GameData, GameView, SgState } from "../types.js";
import { SG_TITLES } from "../text.js";
import { resolveAsset } from "./manifest.js";
import {
  BOARD_W,
  CASINO_CARD,
  C,
  FONT,
  GRAY_DICE_COLOR,
  HEX,
  GAMES,
  USE_BOARD_TEXTURE,
  PLAYER_PANEL,
  POOLS_GAP,
  casinoCenter,
  hexPoints,
  seatColor,
  truncate,
} from "./theme.js";

export interface RectNode {
  t: "rect";
  x: number;
  y: number;
  w: number;
  h: number;
  rx?: number;
  fill: string;
  stroke?: string;
  sw?: number;
  opacity?: number;
}

export interface CircleNode {
  t: "circle";
  cx: number;
  cy: number;
  r: number;
  fill: string;
  stroke?: string;
  sw?: number;
  opacity?: number;
}

export interface PathNode {
  t: "path";
  d: string;
  fill?: string;
  stroke?: string;
  sw?: number;
  opacity?: number;
}

export interface TextNode {
  t: "text";
  x: number;
  y: number;
  text: string;
  size: number;
  fill: string;
  anchor?: "start" | "middle" | "end";
  weight?: number;
  opacity?: number;
}

export interface ImageNode {
  t: "image";
  x: number;
  y: number;
  w: number;
  h: number;
  rx?: number;
  href: string;
  opacity?: number;
}

export type SceneNode = RectNode | CircleNode | PathNode | TextNode | ImageNode;

export interface SceneLayer {
  name: string;
  nodes: SceneNode[];
}

export interface BoardScene {
  w: number;
  h: number;
  layers: SceneLayer[];
  /** 声明需要预加载的素材 URL（首屏只含实际出现的）。 */
  assets: string[];
  meta: {
    round?: number;
    phase?: string;
    phaseLabel?: string;
    turnPlayerId?: string;
    seatCount: number;
  };
}

/** 实物版图 6 家赌场（顺序与 `CASINO_ANGLES` 一致，映射待实物照核定）。 */
export const CASINO_NAMES = ["廣場", "金塊", "海灣", "日落", "奇蹟", "馬戲"];

const PHASE_LABEL: Record<string, string> = {
  lobby: "等待开局",
  setup: "设置中",
  turn_roll: "投掷/放骰",
  small_game: "小游戏",
  round_end: "回合结算",
  settling: "结算中",
  finished: "已结束",
  aborted: "已中止",
};

/** 阶段英文枚举 → 中文（页面顶栏、骰盘、客户端轮询共用）。 */
export function phaseLabel(phase?: string): string {
  if (!phase) return "-";
  return PHASE_LABEL[phase] ?? phase;
}

function text(
  x: number,
  y: number,
  s: string,
  size: number,
  fill: string,
  extra: Partial<TextNode> = {},
): TextNode {
  return { t: "text", x, y, text: s, size, fill, ...extra };
}

function rr(
  x: number,
  y: number,
  w: number,
  h: number,
  rx: number,
  fill: string,
  extra: Partial<RectNode> = {},
): RectNode {
  return { t: "rect", x, y, w, h, rx, fill, ...extra };
}

/** 小游戏在版图上的极简文字摘要（与文本态表格互补，不重复长句）。 */
export function sgSummaryText(id: string, d: GameData, names?: Record<string, string>): string {
  const sg: SgState = d.sg ?? {};
  const nm = (p: string) => truncate(displayName(names, p), 8);
  switch (id) {
    case "F2": {
      const slots = Array.isArray(sg.f2_slots) ? sg.f2_slots : [];
      const filled = slots.filter((v) => v !== null && v !== undefined).length;
      return `${filled}/9 格`;
    }
    case "H1": {
      const slots = Array.isArray(sg.h1_slots) ? sg.h1_slots : [];
      return `${slots.filter(Boolean).length}/6 格`;
    }
    case "G2": {
      const items = Object.entries(sg.game_field ?? {}).filter(([, n]) => Number(n) > 0);
      return items.length ? items.slice(0, 3).map(([p, n]) => `${nm(p)}×${n}`).join("、") : "空";
    }
    case "H2": {
      const gf = sg.gold_frame;
      if (!gf) return "空";
      const bits = [Number(gf.small ?? 0) ? `小${gf.small}` : "", Number(gf.big ?? 0) ? `大${gf.big}` : ""]
        .filter(Boolean)
        .join(" ");
      return `${nm(gf.owner)}${bits ? ` ${bits}` : ""}`;
    }
    case "E2": {
      const items = Object.entries(sg.bars ?? {}).slice(0, 3).map(([p, v]) => {
        const bits = [
          Number(v?.small ?? 0) ? `小${v.small}` : "",
          Number(v?.big ?? 0) ? `大${v.big}` : "",
        ]
          .filter(Boolean)
          .join(" ");
        return `${nm(p)} ${bits || "0"}`;
      });
      return items.length ? items.join("、") : "空";
    }
    case "F1": {
      const g = Array.isArray(sg.gray_groups) ? sg.gray_groups : [];
      return g.length ? g.map((n, i) => `${i + 1}组×${n}`).join("、") : "已用完";
    }
    case "G1": {
      const lot = Array.isArray(sg.lottery) ? sg.lottery : [];
      return `${lot.length} 个`;
    }
    case "A2":
      return `第 ${Number(sg.indicator ?? 0) + 1} 格`;
    case "E1":
      return `${sg.banned === null || sg.banned === undefined ? "未禁止" : `禁 ${Number(sg.banned) + 1} 号`}｜箭 ${Number(sg.arrow ?? 0) + 1}`;
    case "D2":
      return sg.privilege ? nm(sg.privilege) : "在场上";
    case "C1":
      return sg.c1_winner ? `${nm(sg.c1_winner)} 已抢先` : "未触发";
    default:
      return "";
  }
}


/** 各小游戏在板块图上的锚点（0–1，坐标已按实物印位标定）。 */
const TILE_SLOTS: Record<string, Array<[number, number]>> = {
  // F2：3 组（2 / 3 / 4 格）
  F2: [
    [0.27, 0.37],
    [0.35, 0.37],
    [0.53, 0.35],
    [0.6, 0.35],
    [0.67, 0.35],
    [0.32, 0.52],
    [0.41, 0.52],
    [0.5, 0.52],
    [0.59, 0.52],
  ],
  // H1：6 格一排
  H1: [
    [0.13, 0.62],
    [0.28, 0.62],
    [0.41, 0.62],
    [0.53, 0.62],
    [0.65, 0.62],
    [0.77, 0.62],
  ],
  // F1：5 组灰骰
  F1: [
    [0.12, 0.33],
    [0.29, 0.33],
    [0.47, 0.33],
    [0.64, 0.33],
    [0.81, 0.33],
  ],
  // A2：指示物沿钞票格推进
  A2: [
    [0.09, 0.36],
    [0.21, 0.36],
    [0.33, 0.36],
    [0.45, 0.36],
    [0.57, 0.36],
    [0.69, 0.36],
    [0.8, 0.36],
  ],
  // G1：摸彩标记摆在黑条上
  G1: [
    [0.28, 0.38],
    [0.36, 0.38],
    [0.44, 0.38],
    [0.52, 0.38],
    [0.6, 0.38],
  ],
  E2: [
    [0.3, 0.55],
    [0.42, 0.55],
    [0.54, 0.55],
  ],
  E1: [
    [0.5, 0.4],
    [0.72, 0.62],
  ],
  C1: [[0.5, 0.6]],
  D2: [[0.5, 0.5]],
  G2: [[0.55, 0.45]],
  H2: [[0.5, 0.5]],
};

interface TileMarker {
  ux: number;
  uy: number;
  label: string;
  color: string;
  ring?: boolean;
}

/** 把引擎状态映射成「压在板块图上」的标记。 */
function tileMarkers(
  id: string,
  d: GameData,
  view: GameView,
  seatIndex: Map<string, number>,
): TileMarker[] {
  const sg = d.sg ?? {};
  const slots = TILE_SLOTS[id] ?? [];
  const out: TileMarker[] = [];
  const colorOf = (pid: string) =>
    pid === "gray" ? GRAY_DICE_COLOR : seatColor(seatIndex.get(pid) ?? 0);
  const short = (pid: string) => String((seatIndex.get(pid) ?? 0) + 1);
  const at = (i: number): [number, number] => slots[i] ?? slots[slots.length - 1] ?? [0.5, 0.5];

  switch (id) {
    case "F2": {
      const cells = Array.isArray(sg.f2_slots) ? sg.f2_slots : [];
      cells.forEach((v, i) => {
        if (v === null || v === undefined) return;
        const [ux, uy] = at(i);
        out.push({ ux, uy, label: String(v), color: "#2b3644" });
      });
      break;
    }
    case "H1": {
      const cells = Array.isArray(sg.h1_slots) ? sg.h1_slots : [];
      cells.forEach((sl, i) => {
        if (!sl) return;
        const [ux, uy] = at(i);
        out.push({ ux, uy, label: `${short(sl.owner)}`, color: colorOf(sl.owner) });
      });
      break;
    }
    case "A2": {
      const [ux, uy] = at(Math.min(Number(sg.indicator ?? 0), slots.length - 1));
      out.push({ ux, uy, label: `第${Number(sg.indicator ?? 0) + 1}`, color: GRAY_DICE_COLOR, ring: true });
      break;
    }
    case "G1": {
      const lot = Array.isArray(sg.lottery) ? sg.lottery : [];
      lot.forEach((v, i) => {
        const [ux, uy] = at(i);
        out.push({
          ux,
          uy,
          label: Number(v) > 0 ? String(Math.round(Number(v) / 10000)) : "?",
          color: "#11161d",
        });
      });
      break;
    }
    case "G2": {
      const [ux, uy] = at(0);
      const entries = Object.entries(sg.game_field ?? {}).filter(([, n]) => Number(n) > 0);
      entries.forEach(([pid, n], i) => {
        out.push({
          ux: ux + (i - (entries.length - 1) / 2) * 0.09,
          uy,
          label: `${short(pid)}×${n}`,
          color: colorOf(pid),
        });
      });
      break;
    }
    case "H2": {
      const gf = sg.gold_frame;
      if (gf) {
        const [ux, uy] = at(0);
        out.push({ ux, uy, label: `${short(gf.owner)} ${gf.small ?? 0}/${gf.big ?? 0}`, color: colorOf(gf.owner) });
      }
      break;
    }
    case "E2": {
      Object.entries(sg.bars ?? {}).forEach(([pid, v], i) => {
        const [ux, uy] = at(i);
        out.push({ ux, uy, label: `${short(pid)} ${v?.small ?? 0}/${v?.big ?? 0}`, color: colorOf(pid) });
      });
      break;
    }
    case "E1": {
      if (sg.banned !== null && sg.banned !== undefined) {
        const [ux, uy] = at(0);
        out.push({ ux, uy, label: `禁${Number(sg.banned) + 1}`, color: C.danger });
      }
      const [ax, ay] = at(1);
      out.push({ ux: ax, uy: ay, label: `箭${Number(sg.arrow ?? 0) + 1}`, color: "#2b3644" });
      break;
    }
    case "C1": {
      if (sg.c1_winner) {
        const [ux, uy] = at(0);
        out.push({ ux, uy, label: short(sg.c1_winner), color: colorOf(sg.c1_winner) });
      }
      break;
    }
    case "D2": {
      if (sg.privilege) {
        const [ux, uy] = at(0);
        out.push({ ux, uy, label: short(sg.privilege), color: colorOf(sg.privilege) });
      }
      break;
    }
    case "F1": {
      const groups = Array.isArray(sg.gray_groups) ? sg.gray_groups : [];
      groups.forEach((n, i) => {
        const [ux, uy] = at(i);
        out.push({ ux, uy, label: String(n), color: GRAY_DICE_COLOR });
      });
      break;
    }
    default:
      break;
  }
  return out;
}

/** 主入口：把一份 GameView 变成可直接渲染的场景。 */
export function buildBoardScene(view: GameView): BoardScene {
  const d: GameData = view.public?.data ?? {};
  const seats = Array.isArray(d.seats) ? d.seats : [];
  const casinos: CasinoData[] = Array.isArray(d.casinos) ? d.casinos : [];
  const curId = seats[d.turn ?? -1]?.player_id;
  const seatIndex = new Map<string, number>();
  seats.forEach((s, i) => seatIndex.set(s.player_id, i));

  const playerRows = Math.max(1, Math.ceil(seats.length / 2));
  const playerEnd = PLAYER_PANEL.y0 + playerRows * (PLAYER_PANEL.h + PLAYER_PANEL.gapY);
  const mounted = casinos.flatMap((c, i) => (c.small_games ?? []).map((id) => ({ i, id })));
  const gamesTop = playerEnd + 10;
  const gamesH = mounted.length
    ? GAMES.titleH +
      mounted.reduce((acc, { id }) => {
        const a = resolveAsset(id);
        const ratio = a && a.w > 0 ? a.h / a.w : 0.45;
        return acc + Math.round(GAMES.w * ratio) + GAMES.gapY;
      }, 0)
    : 0;
  const poolsY = gamesTop + gamesH + POOLS_GAP;

  const usedAssets: string[] = [];
  const bg: SceneNode[] = [
    rr(0, 0, BOARD_W, 1, 0, C.bg),
    {
      t: "path",
      d: `M ${hexPoints(HEX.cx, HEX.cy, HEX.rOuter).split(" ").join(" L ")} Z`,
      fill: C.board,
      stroke: C.boardEdge,
      sw: 3,
    },
    {
      t: "path",
      d: `M ${hexPoints(HEX.cx, HEX.cy, HEX.rInner).split(" ").join(" L ")} Z`,
      fill: "#0e131a",
      stroke: C.boardEdgeSoft,
      sw: 2,
    },
  ];

  // 实物底盘贴图（六边形已裁好、中央骰盘镂空；有素材才画）
  const boardTex = USE_BOARD_TEXTURE ? resolveAsset("board") : null;
  if (boardTex) {
    bg.push({
      t: "image",
      x: HEX.cx - HEX.rOuter,
      y: HEX.cy - HEX.rOuter,
      w: HEX.rOuter * 2,
      h: HEX.rOuter * 2,
      href: `/board-assets/${boardTex.src}`,
      opacity: 0.5,
    });
    usedAssets.push(`/board-assets/${boardTex.src}`);
  }

  // 中央骰盘信息（轮次 / 阶段 / 当前玩家 / 本轮投掷）
  const roll = d.roll ?? {};
  const tray: SceneNode[] = [
    text(HEX.cx, HEX.cy - 38, `第 ${d.round ?? "-"} 轮`, 26, C.gold, {
      anchor: "middle",
      weight: 700,
    }),
    text(HEX.cx, HEX.cy - 8, phaseLabel(d.phase), FONT.small, C.textMuted, { anchor: "middle" }),
    text(
      HEX.cx,
      HEX.cy + 20,
      curId ? truncate(displayName(view.names, curId), 14) : "等待玩家",
      FONT.small,
      C.text,
      { anchor: "middle" },
    ),
    text(
      HEX.cx,
      HEX.cy + 46,
      Array.isArray(roll.small) && roll.small.length
        ? `骰 ${roll.small.join(" ")}${roll.big ? ` ｜大 ${roll.big}` : ""}`
        : "—",
      FONT.tiny,
      C.textMuted,
      { anchor: "middle" },
    ),
  ];

  // 赌场层
  const casinoNodes: SceneNode[] = [];
  const diceLimit = 3;
  for (let i = 0; i < 6; i += 1) {
    const c = casinos[i] ?? {};
    const { x, y } = casinoCenter(i);
    const left = x - CASINO_CARD.w / 2;
    const top = y - CASINO_CARD.h / 2;
    const hasGame = Array.isArray(c.small_games) && c.small_games.length > 0;

    casinoNodes.push(
      rr(left, top, CASINO_CARD.w, CASINO_CARD.h, CASINO_CARD.r, C.card, {
        stroke: C.cardEdge,
        sw: 1.5,
      }),
    );
    casinoNodes.push({ t: "circle", cx: left + 25, cy: top + 24, r: 14, fill: C.gold });
    casinoNodes.push(
      text(left + 25, top + 31, String(i + 1), FONT.badge, "#1a1206", {
        anchor: "middle",
        weight: 700,
      }),
    );
    casinoNodes.push(
      text(left + 46, top + 31, CASINO_NAMES[i] ?? "", FONT.cardTitle, C.text, { weight: 600 }),
    );

    // 钞票：只显示背面 + 张数（结算前面额脱敏）
    const noteCount = Array.isArray(c.banknotes) ? c.banknotes.length : 0;
    for (let k = 0; k < Math.min(noteCount, 2); k += 1) {
      casinoNodes.push(
        rr(left + CASINO_CARD.w - 24 - k * 8, top + 12 + k * 5, 18, 13, 2, C.note, {
          stroke: C.noteEdge,
          sw: 1,
        }),
      );
      casinoNodes.push(
        text(left + CASINO_CARD.w - 15 - k * 8, top + 22 + k * 5, "?", 10, C.textDim, {
          anchor: "middle",
        }),
      );
    }
    if (noteCount > 0) {
      casinoNodes.push(
        text(left + CASINO_CARD.w - 8, top + 46, `${noteCount}张`, FONT.tiny, C.textDim, {
          anchor: "end",
        }),
      );
    }

    // 骰子（按数量降序，gray 单独配色）
    const dice = c.dice ?? {};
    const big = c.big_dice ?? {};
    const entries = Object.entries(dice)
      .map(([p, n]) => ({ p, n: Number(n) || 0 }))
      .filter((e) => e.n > 0)
      .sort((a, b) => b.n - a.n);
    const shown = entries.slice(0, diceLimit + 1);
    shown.forEach((e, k) => {
      const ry = top + 62 + k * 22;
      const color = e.p === "gray" ? GRAY_DICE_COLOR : seatColor(seatIndex.get(e.p) ?? 0);
      casinoNodes.push(rr(left + 12, ry - 10, 12, 12, 3, color));
      const bi = Number(big[e.p] ?? 0);
      casinoNodes.push(
        text(left + 30, ry, truncate(displayName(view.names, e.p), 13), FONT.small, C.text, {}),
      );
      casinoNodes.push(
        text(left + CASINO_CARD.w - 30, ry, `×${e.n}`, FONT.small, C.textMuted, { anchor: "end" }),
      );
      if (bi > 0) {
        casinoNodes.push(
          rr(left + CASINO_CARD.w - 24, ry - 11, 14, 14, 3, "transparent", {
            stroke: C.gold,
            sw: 2,
          }),
        );
      }
    });
    if (entries.length > shown.length) {
      casinoNodes.push(
        text(left + 12, top + 62 + shown.length * 22, `+${entries.length - shown.length} 家`, FONT.tiny, C.textDim, {}),
      );
    }
    if (!entries.length) {
      casinoNodes.push(text(left + 12, top + 68, "暂无骰子", FONT.small, C.textDim, {}));
    }
  }

  // 玩家面板
  const playerNodes: SceneNode[] = [];
  seats.forEach((s, i) => {
    const col = i % 2;
    const row = Math.floor(i / 2);
    const x = PLAYER_PANEL.x0 + col * (PLAYER_PANEL.w + PLAYER_PANEL.gapX);
    const y = PLAYER_PANEL.y0 + row * (PLAYER_PANEL.h + PLAYER_PANEL.gapY);
    const color = seatColor(i);
    const isTurn = s.player_id === curId;
    const isMe = s.player_id === view.openid;

    playerNodes.push(
      rr(x, y, PLAYER_PANEL.w, PLAYER_PANEL.h, PLAYER_PANEL.r, C.card, {
        stroke: isTurn ? C.turnRing : C.cardEdge,
        sw: isTurn ? 3 : 1.5,
      }),
    );
    playerNodes.push(rr(x + 7, y + 10, 7, PLAYER_PANEL.h - 20, 4, color));
    playerNodes.push(
      text(x + 22, y + 34, truncate(displayName(view.names, s.player_id), 16), 21, C.text, {
        weight: 600,
      }),
    );
    const badges: string[] = [];
    if (isMe) badges.push("我");
    if (s.is_bot) badges.push("机器人");
    if (isTurn) badges.push("行动中");
    if (badges.length) {
      playerNodes.push(
        text(x + 22, y + 60, badges.join(" · "), FONT.tiny, isTurn ? C.accent : C.textDim, {}),
      );
    }
    playerNodes.push(
      text(x + 22, y + 90, `骰 ${s.small_remaining ?? "-"}小 / ${s.big_remaining ?? "-"}大`, FONT.small, C.textMuted, {}),
    );
    playerNodes.push(text(x + 22, y + 118, `筹码 ${s.chips ?? 0}`, FONT.small, C.textMuted, {}));
    const notes = Array.isArray(s.banknotes) ? s.banknotes : [];
    const sum = banknoteTotal(notes);
    playerNodes.push(text(x + 186, y + 90, `钞票 ${notes.length}张`, FONT.small, C.textMuted, {}));
    playerNodes.push(
      text(x + 186, y + 118, sum > 0 ? `$${Math.round(sum / 10000)}万` : "$0", FONT.small, C.gold, {
        weight: 600,
      }),
    );
    if (Array.isArray(s.markers) && s.markers.length) {
      playerNodes.push(text(x + 280, y + 60, `标记${s.markers.length}`, FONT.tiny, C.textDim, {}));
    }
  });

  // 小游戏版面：整幅板块图直接当版面，名称/赌场号/标记全部压在图上的对应位置
  const gameNodes: SceneNode[] = [];
  if (mounted.length) {
    gameNodes.push(
      text(24, gamesTop + 21, `小游戏 ${mounted.length}`, FONT.body, C.textMuted, { weight: 600 }),
    );
    let cursor = gamesTop + GAMES.titleH;
    mounted.forEach(({ i, id }) => {
      const asset = resolveAsset(id);
      const ratio = asset && asset.w > 0 ? asset.h / asset.w : 0.45;
      const imgW = GAMES.w;
      const imgH = Math.round(imgW * ratio);
      const x = GAMES.x0;
      const y = cursor;

      if (asset) {
        gameNodes.push({
          t: "image",
          x,
          y,
          w: imgW,
          h: imgH,
          href: `/board-assets/${asset.src}`,
        });
        usedAssets.push(`/board-assets/${asset.src}`);
      } else {
        gameNodes.push(rr(x, y, imgW, imgH, 10, "#1c2530", { stroke: C.cardEdge, sw: 1.2 }));
      }

      // 标题压条（左上）：引擎名 + id
      gameNodes.push(rr(x + 10, y + 10, 344, 38, 8, "#0b0f14", { opacity: 0.74 }));
      gameNodes.push(
        text(x + 24, y + 36, `${id} ${SG_TITLES[id] ?? ""}`, 22, C.text, { weight: 700 }),
      );
      // 赌场号（右上）
      const chipW = 96;
      gameNodes.push(
        rr(x + imgW - chipW - 12, y + 10, chipW, 32, 8, "#0b0f14", { opacity: 0.72 }),
      );
      gameNodes.push(
        text(x + imgW - chipW / 2 - 12, y + 33, `#${i + 1} 赌场`, FONT.small, C.accent, {
          anchor: "middle",
          weight: 700,
        }),
      );

      // 状态标记（按板块印位摆放）
      const markers = tileMarkers(id, d, view, seatIndex);
      for (const m of markers) {
        const cx = x + m.ux * imgW;
        const cy = y + m.uy * imgH;
        const label = truncate(m.label, 8);
        const pillW = Math.max(34, 22 + Array.from(label).length * 13);
        const pillH = 30;
        gameNodes.push(
          rr(cx - pillW / 2, cy - pillH / 2, pillW, pillH, pillH / 2, m.ring ? "#0b0f14" : m.color, {
            stroke: m.ring ? C.gold : "#0b0f14",
            sw: m.ring ? 3 : 2,
            opacity: m.ring ? 0.85 : 1,
          }),
        );
        gameNodes.push(
          text(cx, cy + 5, label, 14, m.ring ? C.gold : "#e6edf3", {
            anchor: "middle",
            weight: 700,
          }),
        );
      }

      cursor = y + imgH + GAMES.gapY;
    });
  }

  // 公共区（只列“有内容”的项，避免空版面）
  const pools: string[] = [];
  const nm = (p: string) => truncate(displayName(view.names, p), 8);
  const bars = Object.entries(d.sg?.bars ?? {});
  if (bars.length) {
    pools.push(
      `酒吧：${bars
        .slice(0, 3)
        .map(
          ([p, v]) =>
            `${nm(p)} ${
              [Number(v?.small ?? 0) ? `小${v.small}` : "", Number(v?.big ?? 0) ? `大${v.big}` : ""]
                .filter(Boolean)
                .join(" ") || 0
            }`,
        )
        .join("、")}`,
    );
  }
  const lottery = d.sg?.lottery ?? [];
  if (lottery.length) pools.push(`摸彩：${lottery.length} 个`);
  if (d.sg?.banned !== undefined && d.sg?.banned !== null) {
    pools.push(`禁止：${Number(d.sg.banned) + 1} 号赌场`);
  }
  if (d.sg?.privilege) pools.push(`特权：${nm(d.sg.privilege)}`);
  if (d.sg?.c1_winner) pools.push(`掌声响起：${nm(d.sg.c1_winner)} 已抢先`);
  const grayGroups = d.sg?.gray_groups ?? [];
  if (grayGroups.length) {
    pools.push(`灰骰组：${grayGroups.map((n, i) => `${i + 1}组×${n}`).join("、")}`);
  }

  const poolNodes: SceneNode[] = [];
  if (!pools.length) {
    poolNodes.push(text(24, poolsY + 22, "公共区暂无标记", FONT.small, C.textDim, {}));
  } else {
    pools.slice(0, 6).forEach((line, k) => {
      poolNodes.push(text(24, poolsY + 6 + k * 24, truncate(line, 46), FONT.small, C.textMuted, {}));
    });
  }

  const h = poolsY + (pools.length ? Math.min(pools.length, 6) * 24 : 22) + 30;
  bg[0] = rr(0, 0, BOARD_W, h, 0, C.bg);

  // 终局覆盖层
  const finished = d.phase === "finished" || view.public?.status === "finished";
  const overlay: SceneNode[] = finished
    ? [
        rr(0, 0, BOARD_W, h, 0, "#000000", { opacity: 0.34 }),
        text(BOARD_W / 2, HEX.cy - 6, "终局", 56, C.gold, { anchor: "middle", weight: 700 }),
        text(
          BOARD_W / 2,
          HEX.cy + 40,
          truncate((d.winners ?? []).map((w) => displayName(view.names, w)).join("、") || "-", 24),
          24,
          C.text,
          { anchor: "middle" },
        ),
      ]
    : [];

  // 封面小图（有素材才画）
  const cover = resolveAsset("cover");
  const coverNode: SceneNode[] = cover
    ? [
        {
          t: "image",
          x: BOARD_W - 108,
          y: 12,
          w: 92,
          h: 46,
          rx: 8,
          href: `/board-assets/${cover.src}`,
          opacity: 0.92,
        },
      ]
    : [];

  return {
    w: BOARD_W,
    h,
    layers: [
      { name: "bg", nodes: bg },
      { name: "tray", nodes: tray },
      { name: "casinos", nodes: casinoNodes },
      { name: "players", nodes: playerNodes },
      { name: "games", nodes: gameNodes },
      { name: "pools", nodes: poolNodes },
      { name: "overlay", nodes: [...coverNode, ...overlay] },
    ],
    assets: Array.from(new Set(usedAssets)),
    meta: {
      round: d.round,
      phase: d.phase,
      phaseLabel: phaseLabel(d.phase),
      turnPlayerId: curId,
      seatCount: seats.length,
    },
  };
}
