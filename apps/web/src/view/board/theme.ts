// 版图几何与配色常量（纯数据，无 IO）。
//
// 坐标系：逻辑画布 760 宽（手机竖屏优先，等比缩放到容器宽度），高度随玩家数动态计算。
// Web 用 viewBox 等比缩放；RN 用同一份 scene 的绝对坐标 + 等比缩放。
//
// 六边形还原自实物：6 条边 = 6 个赌场（编号 1–6），中间是骰盘；赌场牌挂在外圈，
// 1–3 号赌场外侧可挂小游戏板块（本实现把板块槽放在卡片底部，保证手机可读）。

export const BOARD_W = 760;

/** 六边形棋盘（外框）几何。 */
export const HEX = {
  cx: 380,
  cy: 360,
  rOuter: 340,
  /** 中央骰盘（内六边形）半径。 */
  rInner: 108,
};

/** 赌场牌在六边形外圈上的位置：角度从正上方开始，顺时针每 60°。 */
export const CASINO_ANGLES = [-90, -30, 30, 90, 150, 210] as const;

/** 赌场牌中心到六边形中心的距离。 */
export const CASINO_RING_R = 258;

export const CASINO_CARD = { w: 220, h: 150, r: 12 };

/** 底部长游戏列表（板块缩略图 + id + 赌场号）。 */
/** 是否把实物底盘照片作为底纹（标定未达标前默认关闭）。 */
export const USE_BOARD_TEXTURE = true;

export const GAMES = {
  x0: 20,
  w: 720,
  h: 190,
  gapY: 12,
  thumbW: 280,
  thumbH: 170,
  titleH: 30,
};

/** 玩家面板（2 列）。 */
export const PLAYER_PANEL = {
  x0: 20,
  y0: 748,
  gapX: 16,
  gapY: 14,
  w: 352,
  h: 150,
  r: 12,
};

/** 公共区（游戏场/酒吧/金框/标记……）起始 y 偏移（相对玩家面板结束）。 */
export const POOLS_GAP = 20;

/** 6 人色（色盲友好，深底可见）。 */
export const PLAYER_COLORS = [
  "#e5484d",
  "#0090ff",
  "#30a46c",
  "#f5d90a",
  "#8e4ec6",
  "#f76808",
] as const;

export const GRAY_DICE_COLOR = "#8b949e";

export const C = {
  bg: "#0b0f14",
  board: "#11161d",
  boardEdge: "#c8a24a",
  boardEdgeSoft: "#6b5a26",
  card: "#171e27",
  cardEdge: "#2b3644",
  text: "#e6edf3",
  textMuted: "#98a5b3",
  textDim: "#6e7b8a",
  note: "#1d2530",
  noteEdge: "#3a4757",
  gold: "#e3b341",
  accent: "#f0883e",
  danger: "#f85149",
  ok: "#3fb950",
  turnRing: "#f0883e",
} as const;

export const FONT = {
  title: 30,
  cardTitle: 21,
  badge: 22,
  body: 19,
  small: 17,
  tiny: 15,
} as const;

/** 正六边形顶点（pointy-top，角从 -90° 开始）。 */
export function hexPoints(cx: number, cy: number, r: number): string {
  const pts: string[] = [];
  for (let i = 0; i < 6; i += 1) {
    const a = ((-90 + 60 * i) * Math.PI) / 180;
    pts.push(`${(cx + r * Math.cos(a)).toFixed(2)},${(cy + r * Math.sin(a)).toFixed(2)}`);
  }
  return pts.join(" ");
}

/** 第 i 个赌场牌的中心坐标。 */
export function casinoCenter(i: number): { x: number; y: number } {
  const deg = CASINO_ANGLES[i % 6] ?? -90;
  const a = (deg * Math.PI) / 180;
  return {
    x: HEX.cx + CASINO_RING_R * Math.cos(a),
    y: HEX.cy + CASINO_RING_R * Math.sin(a),
  };
}

/** 玩家色（按座位序号循环；与 RN 端共用）。 */
export function seatColor(seatIndex: number): string {
  const n = PLAYER_COLORS.length;
  return PLAYER_COLORS[((seatIndex % n) + n) % n] as string;
}

/** 按显示宽度截断（CJK 记 2 列，避免文字溢出卡片）。 */
export function truncate(s: string, maxCols: number): string {
  let cols = 0;
  let out = "";
  for (const ch of Array.from(s)) {
    const wide = /[\u2E80-\uFFFF]/.test(ch);
    const w = wide ? 2 : 1;
    if (cols + w > maxCols) return `${out}…`;
    cols += w;
    out += ch;
  }
  return out;
}
