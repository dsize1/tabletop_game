// 观星 2×5 槽（纯函数，P3.5）：初始分布 / 跨列点击 / 同列上移下移 / 拖拽落点 / 结算 / 提交。
//
// 规格：`docs/games/sanguosha/ui-spec.md` §7。
// - 槽位恒为 `L1..L5`（牌堆顶侧）与 `R1..R5`（牌堆底侧），一槽一牌；
// - 初始：`k = min(N,5)` 张依次填 `L1..Lk`，右列为空；
// - 短按：移入另一列自上而下第一个空槽；
// - 拖拽：落空槽放入 / 落有牌槽交换 / 超出吸附阈值回原位；
// - 提交：每列跳空压紧，左列接右列 → `arrange:<csv>`（L1 为新牌堆顶）。

export const GX_ROWS = 5;
export const GX_COLS = 2;
export const GX_SLOT_IDS: string[] = [
  ...Array.from({ length: GX_ROWS }, (_, i) => `L${i + 1}`),
  ...Array.from({ length: GX_ROWS }, (_, i) => `R${i + 1}`),
];
/** 吸附阈值 = 槽短边 × 10%（ui-spec §7.3）。 */
export const GX_SNAP_RATIO = 0.1;

export type GxSlots = Record<string, string | null>;

export function emptySlots(): GxSlots {
  const out: GxSlots = {};
  for (const id of GX_SLOT_IDS) out[id] = null;
  return out;
}

/** 初始分布：依次填 `L1..Lk`。 */
export function initialSlots(cardIds: string[]): GxSlots {
  const slots = emptySlots();
  cardIds.slice(0, GX_ROWS).forEach((id, i) => {
    slots[`L${i + 1}`] = id;
  });
  return slots;
}

function colOf(slot: string): "L" | "R" {
  return slot.startsWith("L") ? "L" : "R";
}

function rowOf(slot: string): number {
  return Number(slot.slice(1));
}

/** 某列自上而下第一个空槽。 */
export function firstEmpty(slots: GxSlots, col: "L" | "R"): string | null {
  for (let i = 1; i <= GX_ROWS; i += 1) {
    const id = `${col}${i}`;
    if (!slots[id]) return id;
  }
  return null;
}

/** 短按：移入另一列第一个空槽；目标列已满则原样返回（不会发生：总牌数 ≤5）。 */
export function applyTap(slots: GxSlots, from: string): GxSlots {
  const card = slots[from];
  if (!card) return slots;
  const target = firstEmpty(slots, colOf(from) === "L" ? "R" : "L");
  if (!target) return slots;
  const next = { ...slots, [from]: null, [target]: card };
  return next;
}

/** 同列上移/下移（与相邻槽交换；空槽也交换，效果等于移动一格）。 */
export function applyMoveWithinColumn(slots: GxSlots, from: string, dir: -1 | 1): GxSlots {
  const row = rowOf(from);
  const to = row + dir;
  if (to < 1 || to > GX_ROWS) return slots;
  const target = `${colOf(from)}${to}`;
  const next = { ...slots };
  const a = next[from];
  const b = next[target];
  next[from] = b;
  next[target] = a;
  return next;
}

/** 拖拽落点：空槽放入 / 有牌交换 / 目标为空或同槽原样返回。 */
export function applyDrop(slots: GxSlots, from: string, to: string | null): GxSlots {
  if (!to || to === from) return slots;
  const card = slots[from];
  if (!card) return slots;
  const next = { ...slots };
  next[to] = card;
  next[from] = slots[to] ?? null;
  return next;
}

/** 结算顺序：每列跳空压紧，左列接右列。 */
export function settle(slots: GxSlots): string[] {
  const out: string[] = [];
  for (const id of GX_SLOT_IDS) {
    const card = slots[id];
    if (card) out.push(card);
  }
  return out;
}

/** 提交动作 id。 */
export function arrangeAction(slots: GxSlots): string {
  return `arrange:${settle(slots).join(",")}`;
}

// ---------------------------------------------------------------------------
// 吸附（几何）
// ---------------------------------------------------------------------------

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** 以槽中心为准取最近槽；距离 > 阈值 → null（回原位）。 */
export function snapSlot(
  point: { x: number; y: number },
  rects: Record<string, Rect>,
  threshold: number,
): string | null {
  let best: string | null = null;
  let bestDist = Number.POSITIVE_INFINITY;
  for (const [slot, r] of Object.entries(rects)) {
    const dx = point.x - (r.left + r.width / 2);
    const dy = point.y - (r.top + r.height / 2);
    const d = Math.hypot(dx, dy);
    if (d < bestDist) {
      bestDist = d;
      best = slot;
    }
  }
  return best && bestDist <= threshold ? best : null;
}

/** 阈值：槽短边 × 10%。 */
export function snapThreshold(rect: Rect): number {
  return Math.min(rect.width, rect.height) * GX_SNAP_RATIO;
}

// ---------------------------------------------------------------------------
// 渲染（浮层内容；纯函数）
// ---------------------------------------------------------------------------

import { escapeHtml } from "../format.js";
import { SUIT_LABEL, cardLabel, rankLabel } from "./format.js";
import type { SgsCard } from "./types.js";

function faceHtml(card: SgsCard): string {
  const red = card.suit === "heart" || card.suit === "diamond" ? " red" : "";
  return (
    `<span class="sgs-gx-face${red}">` +
    `<span class="sgs-gx-rank">${SUIT_LABEL[card.suit]}${rankLabel(card.rank)}</span>` +
    `<span class="sgs-gx-name">${escapeHtml(card.name)}</span>` +
    `</span>`
  );
}

/** 观星浮层内容：2×5 槽 + 操作（重置/确认提交）。 */
export function renderGuanxingOverlay(
  slots: GxSlots,
  cards: SgsCard[],
  opts: { rows?: number } = {},
): string {
  const byId = new Map(cards.map((c) => [c.id, c]));
  const rows = opts.rows ?? GX_ROWS;
  const cell = (slot: string): string => {
    const cardId = slots[slot];
    const card = cardId ? byId.get(cardId) : null;
    const row = rowOf(slot);
    const up = row > 1 ? `<button type="button" class="sgs-gx-move" data-gx-move="up" data-gx-slot="${slot}" aria-label="上移">▲</button>` : "";
    const down = row < rows ? `<button type="button" class="sgs-gx-move" data-gx-move="down" data-gx-slot="${slot}" aria-label="下移">▼</button>` : "";
    if (!card) {
      // 牌面缺失时降级：仍显示「有牌」与 id，避免槽位被误判为空。
      if (cardId) {
        return (
          `<div class="sgs-gx-slot" data-gx-slot="${slot}" data-gx-card="${escapeHtml(cardId)}">` +
          `<span class="sgs-gx-id">${slot}</span><span class="sgs-gx-name">${escapeHtml(cardId)}</span></div>`
        );
      }
      return `<div class="sgs-gx-slot empty" data-gx-slot="${slot}"><span class="sgs-gx-id">${slot}</span></div>`;
    }
    return (
      `<div class="sgs-gx-slot" data-gx-slot="${slot}" data-gx-card="${escapeHtml(card.id)}" ` +
      `title="${escapeHtml(cardLabel(card))}">` +
      `<span class="sgs-gx-id">${slot}</span>` +
      faceHtml(card) +
      (card ? `<span class="sgs-gx-moves">${up}${down}</span>` : "") +
      `</div>`
    );
  };
  const col = (prefix: "L" | "R"): string =>
    Array.from({ length: rows }, (_, i) => cell(`${prefix}${i + 1}`)).join("");
  const order = settle(slots)
    .map((id) => {
      const c = byId.get(id);
      return c ? escapeHtml(cardLabel(c)) : escapeHtml(id);
    })
    .join(" → ");
  const action = arrangeAction(slots);
  return (
    `<p class="muted">点击牌=移到另一列首个空槽；▲▼ 调整同列位置；长按拖动可放入/交换（松手超距回原位）。</p>` +
    `<div class="sgs-gx" data-zone="guanxing">` +
    `<div class="sgs-gx-col"><div class="sgs-gx-col-head">牌堆顶侧（L）</div>${col("L")}</div>` +
    `<div class="sgs-gx-col"><div class="sgs-gx-col-head">牌堆底侧（R）</div>${col("R")}</div>` +
    `</div>` +
    `<p class="muted">最终顺序（顶 → 底）：${order || "无"}</p>` +
    `<div class="sgs-zone-actions">` +
    `<button type="button" class="btn ghost small" data-gx-reset="1">重置</button>` +
    `<button type="button" class="btn small" data-action="${escapeHtml(action)}">确认观星顺序</button>` +
    `</div>`
  );
}
