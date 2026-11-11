// 三国杀浏览器运行时（P3.2）：轮询刷新 + 指针手势 + 居中浮层 + 手牌选中态。
//
// 安全：token 只从页面 bootstrap JSON 读取，随 `/state` 请求传递，不落 localStorage。
// 交互：出牌链路（目标/确认提交）在 P3.3 接线；本文件已提供选中态与详情浮层。

import { escapeHtml } from "../view/format.js";
import { renderSgsApp } from "../view/sgs/page.js";
import { renderSidePanelBody } from "../view/sgs/public.js";
import { applyNames, cardLabel, withDisplayNames } from "../view/sgs/format.js";
import { renderCardDetail, renderDiscardList, renderGeneralDetail } from "../view/sgs/detail.js";
import type { SgsAssetMap, SgsCard, SgsViewData } from "../view/sgs/types.js";
import { buildFxCues } from "../view/sgs/fx.js";
import { awaitingLocal, nextPollDelay, sideSignature, stateSignature } from "../view/sgs/poll.js";
import { enqueueFx, fxLocked, initFx } from "./sgs-fx.js";
import { bindPress } from "./sgs-gestures.js";
import {
  closeOverlay,
  initOverlay,
  isOverlayOpen,
  openOverlay,
  setOverlayClosedHandler,
} from "./sgs-overlay.js";
import {
  GX_ROWS,
  applyDrop,
  applyMoveWithinColumn,
  applyTap,
  draftFromZone,
  initialSlots,
  renderGuanxingOverlay,
  snapSlot,
  snapThreshold,
  type GxSlots,
} from "../view/sgs/guanxing.js";
import {
  isBlockingPending,
  overlayKindFor,
  pendingTitle,
  renderChooseManyOverlay,
  renderCollateralOverlay,
  renderDyingOverlay,
  renderFireAttackOverlay,
  renderPickCardOverlay,
  renderPindianOverlay,
  renderQinwangOverlay,
  renderTargetOverlay,
  renderWuguOverlay,
  renderWuxieOverlay,
} from "../view/sgs/pending.js";

interface Bootstrap {
  gameId: string;
  token: string;
  roomId: string;
  openid: string;
  pollMs?: number;
  /** 武将 code → 卡面（SSR 注入；轮询重渲染沿用，避免头像闪没）。 */
  assets?: SgsAssetMap;
  /** 工作台预览模式（无 token；走预览代理）。 */
  preview?: { room: string; viewer: string; returnUrl?: string };
}

const DEFAULT_POLL_MS = 10_000;
const MAX_POLL_MS = 60_000;
const FETCH_TIMEOUT_MS = 8_000;

let currentData: SgsViewData | null = null;
let currentActions: Array<{ id: string; description: string }> = [];
let appSig = "";
let sideSig = "";
/** `refreshNow`（提交动作后）完成时更新轮询节奏；由 start() 注入。 */
let afterRefreshNow: ((res: StateResponse) => void) | null = null;
let selectedCard: string | null = null;
let unbindPress: (() => void) | null = null;
let submitting = false;
let boot: Bootstrap | null = null;
/** 当前自动打开的 pending 浮层签名（空 = 没有 pending 浮层）。 */
let pendingOverlaySig = "";
/** 因其他浮层（技能/牌详情）打开而延后弹出的 pending 签名（关闭后立即弹）。 */
let pendingDeferredSig = "";
/** 弃牌多选草稿（kind=discard 时点手牌累加，一次确认提交 `discard:<csv>`）。 */
let discardSel: string[] = [];
/** 观星本地槽位（端侧草稿；提交时一次性发 arrange:<csv>）。 */
let gxSlots: GxSlots | null = null;
let gxInitial: GxSlots | null = null;
/** 当前观星草稿对应的特殊区指纹（新一次观星/重置时必须重建草稿）。 */
let gxSig = "";
let gxSuppressClickUntil = 0;
/** 整页刷新/重新进入时恢复的滚动位置（页面 + 历史列表）。 */
let restoreScroll: { page: number; hist: number } | null = null;
let scrollKey = "";
/** 长按后抑制下一次 click（否则技能按钮会同时触发长按详情与点击出牌）。 */
let suppressNextClick = false;
let suppressClickTimer = 0;

function readBootstrap(): Bootstrap | null {
  const node = document.getElementById("sgs-bootstrap");
  if (!node || !node.textContent) return null;
  try {
    return JSON.parse(node.textContent) as Bootstrap;
  } catch {
    return null;
  }
}

function q<T extends HTMLElement>(sel: string): T | null {
  return document.querySelector(sel) as T | null;
}

function setText(id: string, text: string): void {
  const node = document.getElementById(id);
  if (node) node.textContent = text;
}

function hhmmss(d: Date): string {
  const p = (n: number): string => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

// ---------------------------------------------------------------------------
// 数据查找（详情浮层用；只读当前视图）
// ---------------------------------------------------------------------------

function findCard(data: SgsViewData, id: string): SgsCard | null {
  const area = data.player_area;
  const board = data.public_board;
  const pools: Array<SgsCard | null | undefined> = [
    ...(area?.hand ?? []),
    ...Object.values(area?.equips ?? {}),
    ...(area?.judge_zone ?? []),
    ...(board.discard ?? []),
    board.discard_top,
    ...board.seats.flatMap((s) => [...Object.values(s.equips ?? {}), ...(s.judge_zone ?? [])]),
    ...(data.special_zones?.wugu?.cards ?? []),
    ...(data.special_zones?.wugu?.picked ?? []).map((p) => p.card),
    data.special_zones?.judge?.delayed,
    data.special_zones?.judge?.judge_card,
  ];
  for (const c of pools) {
    if (c && c.id === id) return c;
  }
  return null;
}

// ---------------------------------------------------------------------------
// 选中态（P3.2：仅视觉；提交在 P3.3）
// ---------------------------------------------------------------------------

function updateOpsSummary(): void {
  const label = q<HTMLElement>(".sgs-ops .sgs-selected");
  if (!label) return;
  if (!selectedCard || !currentData) {
    label.textContent = "未选择";
    return;
  }
  const card = findCard(currentData, selectedCard);
  label.textContent = card ? `已选：${cardLabel(card)}` : "未选择";
}

/** 选中牌的可用动作（v1 `play:<card>`；v2 `use:<card>[:目标]`；转化技 `convert:*:<card>:*`）。 */
function actionsForCard(card: string): Array<{ id: string; description: string }> {
  return currentActions.filter((a) => {
    if (a.id === `play:${card}` || a.id === `use:${card}`) return true;
    if (a.id.startsWith(`play:${card}:`)) return true;
    if (a.id.startsWith(`use:${card}:`)) return true;
    const m = /^convert:[a-z_]+:([^:]+):/.exec(a.id);
    if (m && m[1] === card) return true;
    const m2 = /^skill:[a-z_]+:(.+)$/.exec(a.id);
    return Boolean(m2 && m2[1] === card);
  });
}

function canPlaySelected(): boolean {
  return Boolean(selectedCard && actionsForCard(selectedCard).length > 0);
}

function syncSelection(): void {
  document.querySelectorAll<HTMLElement>(".sgs-hand .sgs-card").forEach((el) => {
    el.classList.toggle("selected", el.dataset.card === selectedCard);
    el.classList.toggle("discarded-on", discardSel.includes(el.dataset.card ?? ""));
  });
  const confirm = document.querySelector<HTMLButtonElement>('[data-op="confirm"]');
  if (confirm) confirm.disabled = !canPlaySelected();
  const cancel = document.querySelector<HTMLButtonElement>('[data-op="cancel"]');
  if (cancel) cancel.disabled = !selectedCard;
  updateOpsSummary();
}

function toggleSelect(cardId: string): void {
  // 弃牌 pending：多选累加（一次确认提交），同时保留单选草稿供出牌使用。
  if (currentData?.player_area?.pending?.kind === "discard") {
    discardSel = discardSel.includes(cardId)
      ? discardSel.filter((id) => id !== cardId)
      : [...discardSel, cardId];
    syncDiscardSelection();
    return;
  }
  selectedCard = selectedCard === cardId ? null : cardId;
  syncSelection();
}

/**
 * 弃牌多选的就地同步（不重建 `#sgs-app`）：
 * 只切换手牌高亮与「确认弃牌」可用状态，避免选择时整块重绘导致滚动/动画抖动与
 * 元素被替换（连点失效）。
 */
function syncDiscardSelection(): void {
  const discarding = currentData?.player_area?.pending?.kind === "discard";
  document.querySelectorAll<HTMLElement>(".sgs-hand .sgs-card[data-card]").forEach((el) => {
    el.classList.toggle("discardable", discarding);
    el.classList.toggle("discarded-on", discardSel.includes(el.dataset.card ?? ""));
  });
  const bar = document.querySelector<HTMLElement>(".sgs-pending-discard");
  if (!bar) return;
  const required = Number(bar.dataset.required ?? 0);
  const count = bar.querySelector("b");
  if (count) count.textContent = String(discardSel.length);
  const partialOk =
    (currentData?.player_area?.pending?.data as { can_partial?: boolean } | undefined)?.can_partial ===
    true;
  const confirm = bar.querySelector<HTMLButtonElement>('[data-op="discard-confirm"]');
  if (confirm) {
    confirm.disabled = partialOk
      ? !(discardSel.length >= 1 && discardSel.length <= required)
      : discardSel.length !== required;
  }
  const clear = bar.querySelector<HTMLButtonElement>('[data-op="discard-clear"]');
  if (clear) clear.disabled = discardSel.length === 0;
}

// ---------------------------------------------------------------------------
// 动作提交（P3.3）：草稿（选中）→ 按 pending 逐条提交；失败保留草稿并提示
// ---------------------------------------------------------------------------

function readSavedScroll(): { page: number; hist: number } | null {
  if (!scrollKey) return null;
  try {
    const raw = sessionStorage.getItem(scrollKey);
    if (!raw) return null;
    const v = JSON.parse(raw) as { page?: number; hist?: number };
    return { page: Number(v.page) || 0, hist: Number(v.hist) || 0 };
  } catch {
    return null;
  }
}

function saveScroll(): void {
  if (!scrollKey) return;
  try {
    const hist = document.querySelector<HTMLElement>(".sgs-history")?.scrollTop ?? 0;
    sessionStorage.setItem(scrollKey, JSON.stringify({ page: window.scrollY, hist }));
  } catch {
    /* 隐私模式等忽略 */
  }
}

function applyRestoredScroll(): void {
  if (!restoreScroll) return;
  const { page, hist } = restoreScroll;
  restoreScroll = null;
  if (window.scrollY === 0 && page > 0) window.scrollTo(0, page);
  const list = document.querySelector<HTMLElement>(".sgs-history");
  if (list && hist > 0) list.scrollTop = hist;
}

function message(text: string, warn = false): void {
  const node = document.getElementById("sgs-msg");
  if (!node) return;
  node.textContent = text;
  node.classList.toggle("warn", warn);
}

async function submitAction(action: string): Promise<void> {
  if (!boot || submitting || !action || fxLocked()) return;
  // 从「技能选择层/牌详情」等非 pending 浮层提交后必须关闭旧浮层：
  // 否则二阶段 pending（如直谏选目标）不会弹出，且旧按钮可重复提交导致服务端报错（真机反馈）。
  const closeStaleOverlay = isOverlayOpen() && !pendingOverlaySig;
  submitting = true;
  document.body.classList.add("busy");
  try {
    const preview = boot.preview;
    const r = await fetch(
      preview
        ? `/tabletop_game/${encodeURIComponent(boot.gameId)}/preview/action`
        : `/tabletop_game/${encodeURIComponent(boot.gameId)}/action`,
      {
        method: "POST",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify(
          preview
            ? { room: preview.room, player_id: boot.openid, action }
            : { token: boot.token, room_id: boot.roomId, action },
        ),
        cache: "no-store",
      },
    );
    const body = (await r.json().catch(() => ({}))) as {
      ok?: boolean;
      message?: string;
      error?: string;
      step?: { message?: string };
    };
    if (!r.ok) {
      message(body.error ?? `提交失败（${r.status}）`, true);
    } else {
      message(body.step?.message ?? body.message ?? "已提交");
      discardSel = [];
      if (closeStaleOverlay) closeOverlay(true);
    }
    await refreshNow();
  } catch (err) {
    console.error("[sgs] 动作提交失败", err);
    message("连接中断，请稍后重试", true);
  } finally {
    submitting = false;
    document.body.classList.remove("busy");
  }
}

// ---------------------------------------------------------------------------
// pending 浮层（P3.4）：无懈链 / 濒死 / 五谷 / 拿牌 / 目标 / 技能多选
// ---------------------------------------------------------------------------

function pendingSignature(p: NonNullable<SgsViewData["player_area"]>["pending"]): string {
  if (!p) return "";
  return [
    p.kind,
    p.actor,
    p.options.map((o) => o.id).join(","),
    JSON.stringify(p.data ?? {}),
  ].join("|");
}

/** 观星草稿：按特殊区指纹（rev + 牌集 + slots）重建，避免新一次观星复用上次槽位。 */
function syncGuanxingDraft(zone: Parameters<typeof draftFromZone>[0]): void {
  const { slots, sig } = draftFromZone(zone);
  if (gxSlots && gxSig === sig) return;
  gxSig = sig;
  gxSlots = slots;
  gxInitial = { ...slots };
}

function syncPendingOverlay(): void {
  const area = currentData?.player_area;
  const board = currentData?.public_board;
  const p = area?.pending;
  const kind = p ? overlayKindFor(p) : null;
  if (!p || !board || !kind) {
    if (pendingOverlaySig) {
      pendingOverlaySig = "";
      closeOverlay(true);
    }
    gxSlots = null;
    gxInitial = null;
    gxSig = "";
    return;
  }
  const sig = pendingSignature(p);
  if (sig === pendingOverlaySig) return;
  // 已有非 pending 浮层（技能选择/牌详情）打开：不抢占，记录待弹签名，
  // 由浮层关闭回调 `flushDeferredPendingOverlay` 立即弹出。
  if (!pendingOverlaySig && isOverlayOpen()) {
    pendingDeferredSig = sig;
    return;
  }
  pendingOverlaySig = sig;
  pendingDeferredSig = "";
  const blocking = isBlockingPending(p.kind);
  let body = "";
  switch (kind) {
    case "wuxie":
      body = renderWuxieOverlay(p, board, lastNames, currentData?.special_zones);
      break;
    case "dying":
      body = renderDyingOverlay(p, board, lastNames);
      break;
    case "wugu":
      body = renderWuguOverlay(p, board, currentData?.special_zones, lastNames);
      break;
    case "pick_card":
      body = renderPickCardOverlay(p, board, lastNames);
      break;
    case "target":
      body = renderTargetOverlay(p, board, lastNames, (id) => {
        const card = currentData ? findCard(currentData, id) : null;
        return card ? cardLabel(card) : null;
      });
      break;
    case "choose_many":
      body = renderChooseManyOverlay(p, area, lastNames);
      break;
    case "pindian":
      body = renderPindianOverlay(p, board, lastNames);
      break;
    case "fire_attack":
      body = renderFireAttackOverlay(p, board, lastNames);
      break;
    case "collateral":
      body = renderCollateralOverlay(p, board, lastNames);
      break;
    case "qinwang_sha":
      body = renderQinwangOverlay(p, board, lastNames);
      break;
    case "arrange": {
      const zone = currentData?.special_zones?.guanxing;
      syncGuanxingDraft(zone);
      body = renderGuanxingOverlay(gxSlots ?? initialSlots([]), zone?.cards ?? [], {
        rows: zone?.rows ?? GX_ROWS,
      });
      break;
    }
  }
  openOverlay(pendingTitle(p.kind), body, {
    blocking,
    size: kind === "choose_many" || kind === "pick_card" || kind === "arrange" ? "wide" : "normal",
  });
}

/** 浮层关闭后：若有延后的 pending 浮层，立即弹出（避免等下一次轮询）。 */
function flushDeferredPendingOverlay(): void {
  if (!pendingDeferredSig) return;
  pendingDeferredSig = "";
  syncPendingOverlay();
}

function redrawGuanxingOverlay(): void {
  const zone = currentData?.special_zones?.guanxing;
  if (!gxSlots) return;
  openOverlay(
    pendingTitle("arrange"),
    renderGuanxingOverlay(gxSlots, zone?.cards ?? [], { rows: zone?.rows ?? GX_ROWS }),
    { blocking: true, size: "wide" },
  );
}

/** 观星拖拽（一次性绑定在浮层容器上；内容重渲染不影响监听）。 */
function initGuanxingDrag(): void {
  const root = document.getElementById("sgs-overlay");
  if (!root) return;
  let drag: {
    slot: string;
    el: HTMLElement;
    x: number;
    y: number;
    active: boolean;
    timer: number;
    pointerId: number;
  } | null = null;

  const clearTimer = (): void => {
    if (drag?.timer) window.clearTimeout(drag.timer);
    if (drag) drag.timer = 0;
  };

  root.addEventListener("pointerdown", (ev) => {
    if (performance.now() < 0) return;
    const target = ev.target as HTMLElement | null;
    if (!target?.closest(".sgs-gx")) return;
    if (target.closest("[data-gx-move]") || target.closest("[data-gx-reset]")) return;
    const cell = target.closest<HTMLElement>("[data-gx-slot]");
    const slot = cell?.dataset.gxSlot;
    if (!cell || !slot || !cell.dataset.gxCard) return;
    drag = { slot, el: cell, x: ev.clientX, y: ev.clientY, active: false, timer: 0, pointerId: ev.pointerId };
    const current = drag;
    current.timer = window.setTimeout(() => {
      if (!drag || drag !== current) return;
      current.active = true;
      current.el.classList.add("dragging");
      try {
        current.el.setPointerCapture(ev.pointerId);
      } catch {
        /* 合成指针可能不支持捕获 */
      }
    }, 380);
  });

  root.addEventListener("pointermove", (ev) => {
    if (!drag || ev.pointerId !== drag.pointerId) return;
    if (!drag.active) {
      if (Math.hypot(ev.clientX - drag.x, ev.clientY - drag.y) > 8) clearTimer();
      return;
    }
    ev.preventDefault();
    drag.el.style.transform = `translate(${ev.clientX - drag.x}px, ${ev.clientY - drag.y}px)`;
  });

  const finish = (ev: PointerEvent): void => {
    if (!drag || ev.pointerId !== drag.pointerId) return;
    const current = drag;
    drag = null;
    clearTimer();
    current.el.classList.remove("dragging");
    current.el.style.transform = "";
    if (!current.active) return;
    gxSuppressClickUntil = performance.now() + 300;
    const gx = root.querySelector<HTMLElement>(".sgs-gx");
    if (!gx) return;
    const rects: Record<string, { left: number; top: number; width: number; height: number }> = {};
    gx.querySelectorAll<HTMLElement>(".sgs-gx-slot[data-gx-slot]").forEach((el) => {
      const r = el.getBoundingClientRect();
      rects[el.dataset.gxSlot ?? ""] = { left: r.left, top: r.top, width: r.width, height: r.height };
    });
    const first = Object.values(rects)[0];
    const to = snapSlot({ x: ev.clientX, y: ev.clientY }, rects, first ? snapThreshold(first) : 0);
    if (gxSlots && to) {
      gxSlots = applyDrop(gxSlots, current.slot, to);
      redrawGuanxingOverlay();
    }
  };
  root.addEventListener("pointerup", finish);
  root.addEventListener("pointercancel", finish);
}

// ---------------------------------------------------------------------------
// 手势 / 浮层
// ---------------------------------------------------------------------------

function onTap(el: HTMLElement): void {
  if (fxLocked()) return;
  const kind = el.dataset.press;
  if (kind === "hand" && el.dataset.card) {
    toggleSelect(el.dataset.card);
    return;
  }
  if (kind === "general") {
    const seatIdx = Number(el.dataset.seat);
    const seat = currentData?.public_board.seats[seatIdx];
    if (seat) openOverlay("武将详情", renderGeneralDetail(seat, lastNames, boot?.assets));
    return;
  }
  if (kind === "discard" && currentData) {
    openOverlay("弃牌堆", renderDiscardList(currentData.public_board));
    return;
  }
  if (kind === "card" && el.dataset.card && currentData) {
    const card = findCard(currentData, el.dataset.card);
    if (card) openOverlay(cardLabel(card), renderCardDetail(card));
    return;
  }
  if (kind === "skill") {
    // 有直接动作的技能按钮：交给 click（data-action）提交，不再弹选择层。
    if (el.dataset.action) return;
    const raw = el.dataset.conversions;
    if (!raw) return;
    try {
      const list = JSON.parse(raw) as Array<{ id: string; label: string }>;
      const buttons = list
        .map(
          (a) =>
            `<button type="button" class="btn small" data-action="${escapeHtml(a.id)}">` +
            `${escapeHtml(a.label)}</button>`,
        )
        .join("");
      openOverlay(
        `技能 · ${el.dataset.skillName ?? ""}`,
        `<p class="muted">选择要使用的牌：</p><div class="sgs-zone-actions">${buttons}</div>`,
      );
    } catch {
      message("技能动作解析失败", true);
    }
  }
}

function onClick(ev: MouseEvent): void {
  if (fxLocked()) return;
  if (suppressNextClick) {
    suppressNextClick = false;
    return;
  }
  const target = ev.target as HTMLElement | null;
  const opEl = target?.closest?.("[data-op]") as HTMLElement | null;
  if (opEl) {
    const op = opEl.dataset.op;
    if (op === "discard-confirm") {
      const p = currentData?.player_area?.pending;
      const required = Number((p?.data as { required?: number } | undefined)?.required ?? 0);
      if (!p || p.kind !== "discard") return;
      if (discardSel.length === 0) {
        message("请先选择要弃置的牌", true);
        return;
      }
      const partialOk = (p.data as { can_partial?: boolean } | undefined)?.can_partial === true;
      if (discardSel.length > required) {
        message(`最多弃 ${required} 张`, true);
        return;
      }
      if (!partialOk && discardSel.length !== required) {
        // 庸肆等固定张数弃牌：必须一次凑齐（少选会被服务端拒绝，表现为「反复要求弃 X 张」）。
        message(`需一次选择 ${required} 张`, true);
        return;
      }
      const csv = discardSel.join(",");
      discardSel = [];
      void submitAction(`discard:${csv}`);
      return;
    }
    if (op === "discard-clear") {
      discardSel = [];
      syncDiscardSelection();
      return;
    }
    if (op === "confirm") {
      if (!selectedCard) {
        message("请先选择一张手牌", true);
        return;
      }
      // 直接使用（无目标）：v1 `play:<card>` / v2 `use:<card>`。
      for (const id of [`play:${selectedCard}`, `use:${selectedCard}`]) {
        if (currentActions.some((a) => a.id === id)) {
          void submitAction(id);
          return;
        }
      }
      const candidates = actionsForCard(selectedCard);
      if (candidates.length === 0) {
        message("该牌当前不可打出", true);
        return;
      }
      if (candidates.length === 1) {
        void submitAction(candidates[0].id);
        return;
      }
      // v2：需要目标 / 转化方式有多选 → 弹选择层（按钮都是可直接提交的 action id）。
      const buttons = candidates
        .map(
          (a) =>
            `<button type="button" class="btn small" data-action="${escapeHtml(a.id)}">` +
            `${escapeHtml(a.description)}</button>`,
        )
        .join("");
      openOverlay(
        "选择目标 / 用法",
        `<p class="muted">这张牌有多种用法：</p><div class="sgs-zone-actions">${buttons}</div>`,
      );
    } else if (op === "cancel") {
      selectedCard = null;
      syncSelection();
    }
    return;
  }
  // 观星槽位交互（只在浮层内）
  const gxEl = target?.closest?.("[data-gx-slot], [data-gx-reset], [data-gx-move]") as HTMLElement | null;
  if (gxEl && gxSlots) {
    if (performance.now() < gxSuppressClickUntil) return; // 拖拽结束抑制一次点击
    if (gxEl.dataset.gxReset) {
      gxSlots = { ...(gxInitial ?? gxSlots) };
    } else if (gxEl.dataset.gxMove) {
      gxSlots = applyMoveWithinColumn(
        gxSlots,
        gxEl.dataset.gxSlot ?? "",
        gxEl.dataset.gxMove === "up" ? -1 : 1,
      );
    } else {
      gxSlots = applyTap(gxSlots, gxEl.dataset.gxSlot ?? "");
    }
    redrawGuanxingOverlay();
    return;
  }
  const actionEl = target?.closest?.("[data-action]") as HTMLElement | null;
  const action = actionEl?.dataset.action;
  if (action) {
    void submitAction(action);
  }
}

function onLongPress(el: HTMLElement, ev: PointerEvent): void {
  // 长按先给反馈（ui-spec §5.8）；手牌/装备/判定/技能都开详情。
  const kind = el.dataset.press;
  if ((kind === "hand" || kind === "card") && el.dataset.card && currentData) {
    const card = findCard(currentData, el.dataset.card);
    if (card) {
      ev.preventDefault();
      openOverlay(cardLabel(card), renderCardDetail(card));
    }
    return;
  }
  if (kind === "skill") {
    const name = el.dataset.skillName ?? "技能";
    const desc = el.dataset.desc ?? "";
    ev.preventDefault();
    suppressNextClick = true;
    window.clearTimeout(suppressClickTimer);
    suppressClickTimer = window.setTimeout(() => {
      suppressNextClick = false;
    }, 1500);
    const text = escapeHtml(desc).replace(/\n/g, "<br>");
    const body = desc
      ? `<p class="sgs-skill-desc">${text}</p>`
      : `<p class="muted">暂无技能简述</p>`;
    openOverlay(`技能 · ${name}`, body);
  }
}

let lastNames: Record<string, string> | undefined;

function bindInteractions(): void {
  const app = q<HTMLElement>("#sgs-app");
  if (!app) return;
  unbindPress?.();
  unbindPress = bindPress(app, { onTap, onLongPress });
}

// ---------------------------------------------------------------------------
// 轮询
// ---------------------------------------------------------------------------

interface StateResponse {
  public?: { data?: SgsViewData };
  player?: { data?: SgsViewData };
  names?: Record<string, string>;
  actions?: Array<{ id: string; description: string }>;
  error?: string;
}

async function fetchState(boot: Bootstrap): Promise<StateResponse> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const preview = boot.preview;
    const url = preview
      ? `/tabletop_game/${encodeURIComponent(boot.gameId)}/preview/state` +
        `?room=${encodeURIComponent(preview.room)}&viewer=${encodeURIComponent(boot.openid)}`
      : `/tabletop_game/${encodeURIComponent(boot.gameId)}/state?token=${encodeURIComponent(boot.token)}`;
    const r = await fetch(url, {
      signal: ctrl.signal,
      headers: { accept: "application/json" },
      cache: "no-store",
    });
    if (!r.ok) throw new Error(`state ${r.status}`);
    return (await r.json()) as StateResponse;
  } finally {
    clearTimeout(timer);
  }
}

function applyState(res: StateResponse): void {
  const raw = res.player?.data ?? res.public?.data;
  if (!raw) return;
  // 引擎只认 openid/`bot:N`；渲染前把**文案字段**换成群昵称（日志/待响应选项）；
  // 动作描述（legal_actions.description）同理——否则端侧会出现 `BE6B1438… 造成 1 点伤害`。
  const data = withDisplayNames(raw, res.names);
  const prev = currentData;
  const nextActions = (res.actions ?? []).map((a) => ({
    ...a,
    description: applyNames(a.description, res.names),
  }));
  const nextAppSig = stateSignature(data, nextActions, res.names);
  const nextSideSig = sideSignature(data, res.names);
  currentData = data;
  currentActions = nextActions;
  lastNames = res.names;
  // 弃牌草稿：pending 变化/结束时清空；仍为弃牌则保留仍在手牌的选中项。
  if (data.player_area?.pending?.kind === "discard") {
    const hand = new Set((data.player_area.hand ?? []).map((c) => c.id));
    discardSel = discardSel.filter((id) => hand.has(id));
  } else {
    discardSel = [];
  }
  // 选中的牌可能已被打出/弃置：不再存在于手牌则清空。
  if (selectedCard && !(data.player_area?.hand ?? []).some((c) => c.id === selectedCard)) {
    selectedCard = null;
  }
  // 只在应用态变化时重建 #sgs-app（避免无意义闪屏；history 变化只刷侧栏）。
  const app = q<HTMLElement>("#sgs-app");
  if (app && nextAppSig !== appSig) {
    appSig = nextAppSig;
    app.innerHTML = renderSgsApp(data, res.names, currentActions, boot?.assets, discardSel);
    bindInteractions();
    syncSelection();
  }
  const board = data.public_board;
  const meta = document.querySelector(".sgs-topbar .meta");
  if (meta && board) meta.textContent = `第 ${board.round} 轮 · ${board.turn.player_id ?? "-"}`;
  // 侧栏（阶段/本局/出牌历史）不在 #sgs-app 内，需单独刷新。
  const side = document.getElementById("sgs-side");
  if (side && nextSideSig !== sideSig) {
    sideSig = nextSideSig;
    const oldList = side.querySelector<HTMLElement>(".sgs-history");
    const keepScroll = oldList?.scrollTop ?? 0;
    side.innerHTML = renderSidePanelBody(data, res.names);
    const newList = side.querySelector<HTMLElement>(".sgs-history");
    if (newList && keepScroll > 0) newList.scrollTop = keepScroll;
  }
  syncPendingOverlay();
  // 特效：首帧 prev=null 只建基线，不回放旧历史；浮层/隐藏时由 fx 模块跳过。
  const cues = buildFxCues(prev, data);
  if (cues.length > 0) enqueueFx(cues);
}

async function refreshNow(): Promise<void> {
  if (!boot) return;
  const res = await fetchState(boot);
  // 始终应用最新状态（公共区/待响应提示必须随对局推进更新）；
  // 浮层是否被抢占由 syncPendingOverlay 内的「延后弹出」策略决定（真机：兵粮判定残留）。
  applyState(res);
  applyRestoredScroll();
  setText("sgs-warn", "");
  setText("sgs-updated", `更新于 ${hhmmss(new Date())}`);
  afterRefreshNow?.(res);
}

/** 工作台预览工具栏：视角切换 + AI 单步/连推 + 重置（仅预览页存在）。 */
async function previewCommand(cmd: string): Promise<void> {
  if (!boot?.preview || submitting || fxLocked()) return;
  const room = boot.preview.room;
  const base = `/tabletop_game/${encodeURIComponent(boot.gameId)}/preview`;
  submitting = true;
  document.body.classList.add("busy");
  try {
    if (cmd === "reset") {
      const r = await fetch(`${base}/reset`, {
        method: "POST",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify({ room }),
        cache: "no-store",
      });
      const body = (await r.json().catch(() => ({}))) as { error?: string };
      if (!r.ok) {
        message(body.error ?? `重置失败（${r.status}）`, true);
        return;
      }
      message("已回到模板基线");
      await refreshNow();
      return;
    }
    const max_steps = cmd === "ai-drive" ? 50 : 1;
    const r = await fetch(`${base}/drive`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ room, max_steps, viewer: boot.openid }),
      cache: "no-store",
    });
    const body = (await r.json().catch(() => ({}))) as {
      error?: string;
      steps?: number;
      stop?: string;
      trace?: { action?: string; reason?: string; score?: number };
    };
    if (!r.ok) {
      message(body.error ?? `AI 推进失败（${r.status}）`, true);
      return;
    }
    const trace = body.trace;
    const detail = trace?.action
      ? `AI：${trace.action}${trace.reason ? `（${trace.reason}）` : ""}`
      : `AI 推进 ${body.steps ?? 0} 步`;
    message(`${detail}${body.stop ? ` · 停止：${body.stop}` : ""}`);
    await refreshNow();
  } catch (err) {
    console.error("[sgs] 预览操作失败", err);
    message("连接中断，请稍后重试", true);
  } finally {
    submitting = false;
    document.body.classList.remove("busy");
  }
}

function bindPreviewControls(): void {
  if (!boot?.preview) return;
  const select = q<HTMLSelectElement>("#sgs-preview-viewer");
  select?.addEventListener("change", () => {
    const viewer = select.value;
    const url =
      `/tabletop_game/${encodeURIComponent(boot!.gameId)}/preview` +
      `?room=${encodeURIComponent(boot!.preview!.room)}&viewer=${encodeURIComponent(viewer)}`;
    window.location.href = url;
  });
  document.querySelectorAll<HTMLElement>("[data-preview-cmd]").forEach((el) => {
    el.addEventListener("click", () => void previewCommand(el.dataset.previewCmd ?? ""));
  });
}

function start(): void {
  const bootstrap = readBootstrap();
  if (!bootstrap) return;
  boot = bootstrap;
  scrollKey = `sgs:scroll:${bootstrap.roomId}`;
  restoreScroll = readSavedScroll();
  window.addEventListener("pagehide", saveScroll);
  window.addEventListener("beforeunload", saveScroll);
  initOverlay();
  setOverlayClosedHandler(() => {
    flushDeferredPendingOverlay();
    // 关闭详情/技能层后刷新一次：可能在浮层打开期间错过了 pending 变化。
    void refreshNow();
  });
  bindInteractions();
  bindPreviewControls();
  // 点击/拖拽委托只绑一次（容器元素不变，innerHTML 重渲染不影响）
  q<HTMLElement>("#sgs-app")?.addEventListener("click", onClick);
  initGuanxingDrag();
  q<HTMLElement>("#sgs-overlay")?.addEventListener("click", onClick);

  const basePollMs = bootstrap.pollMs ?? DEFAULT_POLL_MS;
  let pollMs = basePollMs;
  let timer: number | undefined;
  let pausedForFx = false;

  const schedule = (): void => {
    if (pausedForFx) return;
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(refresh, pollMs);
  };

  const refresh = async (): Promise<void> => {
    if (document.hidden) {
      schedule();
      return;
    }
    try {
      const res = await fetchState(bootstrap);
      // 详情浮层打开时不打断用户；pending 浮层需要随局面更新（等待他人/链推进）。
      if (!isOverlayOpen() || pendingOverlaySig) applyState(res);
      const data = res.player?.data ?? res.public?.data;
      pollMs = nextPollDelay(data, res.actions, basePollMs);
      setText("sgs-warn", "");
      setText("sgs-updated", `更新于 ${hhmmss(new Date())}`);
      if (!fxLocked()) {
        setText("sgs-sync", awaitingLocal(data, res.actions) ? "等待你的操作" : "已同步");
      }
    } catch {
      pollMs = Math.min(MAX_POLL_MS, pollMs * 2);
      setText("sgs-warn", "连接中断");
    } finally {
      schedule();
    }
  };

  initFx({
    getSelfSeat: () => currentData?.player_area?.seat ?? null,
    onLockChange: (locked) => {
      pausedForFx = locked;
      document.body.classList.toggle("sgs-fx-locked", locked);
      if (locked) {
        if (timer !== undefined) window.clearTimeout(timer);
        timer = undefined;
        setText("sgs-sync", "行动结算中…");
      } else {
        setText("sgs-sync", "已同步");
        schedule();
      }
    },
    onDrain: () => {
      // 队列结束：拉一次最新状态（可能已有下一次 pending），再恢复轮询节奏。
      pausedForFx = false;
      void refresh();
    },
  });

  // 提交动作 / 预览命令后的 refreshNow 不经过 refresh 的 finally：这里同步轮询节奏并重新排程，
  // 避免「原 60s 等待定时器」在动作完成后继续拖延下一次轮询。
  afterRefreshNow = (res) => {
    const data = res.player?.data ?? res.public?.data;
    pollMs = nextPollDelay(data, res.actions, basePollMs);
    if (!fxLocked()) {
      setText("sgs-sync", awaitingLocal(data, res.actions) ? "等待你的操作" : "已同步");
      schedule();
    }
  };

  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) void refresh();
  });
  window.addEventListener("focus", () => void refresh());
  window.addEventListener("pageshow", (e) => {
    if ((e as PageTransitionEvent).persisted) void refresh();
  });
  window.addEventListener("online", () => void refresh());

  setText("sgs-updated", `更新于 ${hhmmss(new Date())}`);
  // 首屏是 SSR：立即拉一次 `/state` 填充客户端数据（详情/选中依赖 currentData），
  // 然后进入正常轮询节奏（refresh 内部会 schedule）。
  void refresh();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start);
} else {
  start();
}

// 供 P3.3 复用的内部能力（提交/刷新）
export { closeOverlay };
