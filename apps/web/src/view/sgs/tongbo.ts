// 通博（`tongbo`）两列交换交互（V3 引擎，2026-11-16）。
//
// 规格（用户裁定）：
// - 自己回合**摸牌阶段结束**时询问是否发动；
// - 多段：①「书 / 手牌」左右两列交换（每次拖拽一对，可反复）＋确认按钮；
//         ②确认后若「书」覆盖四花色 → **必须**将所有「书」交给一名其他角色（不能放弃），否则结束；
// - **装备区不参与**：只有手牌能进「书」。
//
// 引擎协议（`kernel/skills/b8.rs::tongbo`）：
// - 交换：pending `kind = "tongbo_swap"`，`data.books/hand` 为两列牌 id、`data.labels` 为牌面名；
//   一次拖拽提交 `swap:<bookId>:<handId>`；「确认交换」提交出口 `pass`（非取消语义）；
// - 交出：pending `kind = "target"` 且 `skill = "tongbo"`（强制选择，不可放弃）。

import { escapeHtml } from "../format.js";

export const TB_KIND = "tongbo_swap";
export const TB_ZONE = "tongbo";

export type TbColumn = "book" | "hand";

export interface TbCardRef {
  col: TbColumn;
  id: string;
}

/**
 * 一次拖拽 → 交换动作 id（`swap:<book>:<hand>`）。
 *
 * 只在「跨列 + 两端都有牌」时成立（左右交换保持两侧张数不变，规格要求）。
 */
export function tongboSwapAction(from: TbCardRef, to: TbCardRef): string | null {
  if (!from.id || !to.id || from.col === to.col) return null;
  const book = from.col === "book" ? from.id : to.id;
  const hand = from.col === "book" ? to.id : from.id;
  if (!book || !hand) return null;
  return `swap:${book}:${hand}`;
}

export interface TbColumns {
  books: string[];
  hand: string[];
  labels: Record<string, string>;
}

/** 从 pending.data 取两列（容错：缺失时返回空列）。 */
export function tongboColumns(data: unknown): TbColumns {
  const d = (data ?? {}) as { books?: unknown; hand?: unknown; labels?: unknown };
  const ids = (v: unknown): string[] =>
    Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
  const labels =
    d.labels && typeof d.labels === "object"
      ? (d.labels as Record<string, string>)
      : {};
  return { books: ids(d.books), hand: ids(d.hand), labels };
}

/** 渲染两列（左「书」/右「手牌」）+ 确认按钮；复用观星槽位样式。 */
export function renderTongboOverlay(cols: TbColumns): string {
  const card = (col: TbColumn, id: string): string => {
    const label = cols.labels[id] ?? id;
    return (
      `<div class="sgs-gx-slot" data-tb-col="${col}" data-tb-card="${escapeHtml(id)}" ` +
      `title="${escapeHtml(label)}">` +
      `<span class="sgs-gx-name">${escapeHtml(label)}</span>` +
      `</div>`
    );
  };
  const dense = (col: TbColumn, list: string[]): string =>
    list.length
      ? list.map((id) => card(col, id)).join("")
      : `<div class="sgs-gx-slot empty"><span class="sgs-gx-id">（空）</span></div>`;
  return (
    `<p class="muted">通博：把「书」（左）与手牌（右）拖拽交换，可反复；确认后若「书」含四种花色，必须全部交给一名其他角色。</p>` +
    `<div class="sgs-gx" data-zone="${TB_ZONE}">` +
    `<div class="sgs-gx-col"><div class="sgs-gx-col-head">书（武将牌上）</div>${dense("book", cols.books)}</div>` +
    `<div class="sgs-gx-col"><div class="sgs-gx-col-head">手牌</div>${dense("hand", cols.hand)}</div>` +
    `</div>` +
    `<div class="sgs-zone-actions">` +
    `<button type="button" class="btn small" data-action="pass">确认交换</button>` +
    `</div>`
  );
}

/** 一次拖拽的落点解析：优先按指针下的牌取列与 id。 */
export function tongboDropTarget(
  el: { getAttribute(name: string): string | null } | null,
): TbCardRef | null {
  if (!el) return null;
  const col = el.getAttribute("data-tb-col");
  const id = el.getAttribute("data-tb-card");
  if ((col !== "book" && col !== "hand") || !id) return null;
  return { col, id };
}
