// 三国杀视图：文案 / 牌面 / 座位布局（纯函数，服务端与浏览器共用）。

import { escapeHtml } from "../format.js";
import type { SgsCard, Suit } from "./types.js";

export const SUIT_LABEL: Record<Suit, string> = {
  spade: "♠",
  heart: "♥",
  club: "♣",
  diamond: "♦",
};

export const SUIT_RED: Record<Suit, boolean> = {
  spade: false,
  heart: true,
  club: false,
  diamond: true,
};

/** 手牌上限展示：`null` = 本回合无上限。 */
export function handLimitLabel(limit?: number | null): string {
  return limit === null || limit === undefined ? "∞" : String(limit);
}

/** 点数：1→A、11→J、12→Q、13→K。 */
export function rankLabel(rank: number): string {
  if (rank === 1) return "A";
  if (rank === 11) return "J";
  if (rank === 12) return "Q";
  if (rank === 13) return "K";
  return String(rank);
}

export const PHASE_LABEL: Record<string, string> = {
  lobby: "等待加入",
  setup: "选将",
  turn: "对局中",
  dying: "濒死",
  finished: "已结束",
  prepare: "准备",
  judge: "判定",
  draw: "摸牌",
  play: "出牌",
  discard: "弃牌",
  end: "结束",
};

export const TURN_PHASE_ORDER = ["prepare", "judge", "draw", "play", "discard", "end"];

export const IDENTITY_LABEL: Record<string, string> = {
  lord: "主公",
  loyalist: "忠臣",
  rebel: "反贼",
  spy: "内奸",
};

export const KINGDOM_LABEL: Record<string, string> = {
  shu: "蜀",
  wei: "魏",
  wu: "吴",
  qun: "群",
};

export const EQUIP_SLOT_LABEL: Record<string, string> = {
  weapon: "武器",
  armor: "护具",
  horse_offense: "进攻马",
  horse_defense: "防御马",
  treasure: "特殊",
};

export const EQUIP_SLOT_ORDER = [
  "weapon",
  "armor",
  "horse_offense",
  "horse_defense",
  "treasure",
] as const;

/** 判定区一字图标（ui-spec §4：乐/兵/雷，后进的在左边）。 */
export function judgeIcon(card: SgsCard): string {
  switch (card.def) {
    case "lebu_sishu":
      return "乐";
    case "bingliang_cunduan":
      return "兵";
    case "shandian":
      return "雷";
    default:
      return (card.name ?? "?").slice(0, 1);
  }
}

export function cardLabel(card: SgsCard): string {
  return `${card.name}${SUIT_LABEL[card.suit] ?? ""}${rankLabel(card.rank)}`;
}

/** 紧凑牌面：点数 + 花色 + 牌名（个人手牌）。 */
export function cardFaceHtml(card: SgsCard, extraClass = ""): string {
  const red = SUIT_RED[card.suit] ? " red" : "";
  return (
    `<span class="sgs-card${red}${extraClass}" data-press="hand" data-card="${escapeHtml(card.id)}" title="${escapeHtml(cardLabel(card))}">` +
    `<span class="sgs-card-rank">${rankLabel(card.rank)}${SUIT_LABEL[card.suit] ?? ""}</span>` +
    `<span class="sgs-card-name">${escapeHtml(card.name)}</span>` +
    `</span>`
  );
}

/** 背面牌（他人手牌）。 */
export function cardBackHtml(): string {
  return `<span class="sgs-card back" aria-label="手牌背面"></span>`;
}

/** 小标记（装备/判定）；`long` 时带牌名。 */
export function cardChipHtml(card: SgsCard, opts: { icon?: string; long?: boolean } = {}): string {
  const red = SUIT_RED[card.suit] ? " red" : "";
  const text = opts.icon
    ? `${opts.icon}`
    : opts.long
      ? cardLabel(card)
      : `${rankLabel(card.rank)}${SUIT_LABEL[card.suit] ?? ""}`;
  return (
    `<span class="sgs-chip${red}" data-press="card" data-card="${escapeHtml(card.id)}" ` +
    `title="${escapeHtml(cardLabel(card))}">${escapeHtml(text)}</span>`
  );
}

/** 显示名（群昵称优先，缺失回退 openid）。 */
export function playerName(names: Record<string, string> | undefined, id: string): string {
  return names?.[id] ?? id;
}

export function hpPips(hp: number, maxHp: number): string {
  const total = Math.max(0, maxHp);
  let out = "";
  for (let i = 0; i < total; i += 1) {
    out += i < hp ? `<i class="hp on"></i>` : `<i class="hp off"></i>`;
  }
  return out;
}

// ---------------------------------------------------------------------------
// 座位布局（ui-spec §3.2）：他人数量 m = 人数-1 → 左/上/右；百分比定位
// ---------------------------------------------------------------------------

export interface SeatPlacement {
  /** 在「他人」数组中的下标（0 = 自己左手侧第一位）。 */
  index: number;
  /** 绝对定位百分比：left/top 为卡牌左上角；`center` 表示需 translateX(-50%)。 */
  left: number;
  top: number;
  side: "left" | "top" | "right";
}

/** 按人数拆分（8 人基准 2/3/2）。 */
export function splitSides(m: number): { left: number; top: number; right: number } {
  const top = Math.min(3, Math.ceil(m / 2));
  const left = Math.ceil((m - top) / 2);
  const right = m - top - left;
  return { left, top, right };
}

/**
 * 他人座位布局：填位顺序 = 自己左手侧起逆时针（左列自下而上 → 上排自左而右 → 右列自上而下）。
 */
export function seatPlacements(m: number): SeatPlacement[] {
  const { left, top, right } = splitSides(m);
  const out: SeatPlacement[] = [];
  for (let i = 0; i < left; i += 1) {
    const l = left;
    // 自下而上：第一个在底部
    const ratio = l === 1 ? 0.42 : i === 0 ? 0.62 : 0.28;
    out.push({ index: i, left: 2, top: Math.round(ratio * 100), side: "left" });
  }
  const topX = (k: number): number => {
    if (top === 1) return 50;
    if (top === 2) return [28, 66][k] ?? 50;
    return [14, 43, 72][k] ?? 50;
  };
  for (let i = 0; i < top; i += 1) {
    out.push({
      index: left + i,
      left: topX(i),
      top: 1,
      side: "top",
    });
  }
  for (let i = 0; i < right; i += 1) {
    const ratio = right === 1 ? 0.42 : i === 0 ? 0.18 : 0.52;
    out.push({
      index: left + top + i,
      left: 98,
      top: Math.round(ratio * 100),
      side: "right",
    });
  }
  return out;
}

/** 自己左手侧的他人顺序（座位号数组）。 */
export function othersInSeatOrder(n: number, mySeat: number): number[] {
  const out: number[] = [];
  for (let k = 1; k < n; k += 1) out.push((mySeat + k) % n);
  return out;
}

/** 「武将牌上的标记」展示名（未知 code 原样展示）。 */
export const MARK_LABEL: Record<string, string> = {
  buqu: "创",
  tian: "田",
  ju: "橘",
  ren: "任",
  rong: "荣",
  huanhua: "化身",
};
