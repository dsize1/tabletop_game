// 三国杀浏览器运行时（P3.2）：轮询刷新 + 指针手势 + 居中浮层 + 手牌选中态。
//
// 安全：token 只从页面 bootstrap JSON 读取，随 `/state` 请求传递，不落 localStorage。
// 交互：出牌链路（目标/确认提交）在 P3.3 接线；本文件已提供选中态与详情浮层。

import { renderSgsApp } from "../view/sgs/page.js";
import { cardLabel } from "../view/sgs/format.js";
import { renderCardDetail, renderDiscardList, renderGeneralDetail } from "../view/sgs/detail.js";
import type { SgsCard, SgsViewData } from "../view/sgs/types.js";
import { bindPress } from "./sgs-gestures.js";
import { closeOverlay, initOverlay, isOverlayOpen, openOverlay } from "./sgs-overlay.js";
import {
  GX_ROWS,
  applyDrop,
  applyMoveWithinColumn,
  applyTap,
  initialSlots,
  renderGuanxingOverlay,
  settle,
  snapSlot,
  snapThreshold,
  type GxSlots,
} from "../view/sgs/guanxing.js";
import {
  isBlockingPending,
  overlayKindFor,
  pendingTitle,
  renderChooseManyOverlay,
  renderDyingOverlay,
  renderPickCardOverlay,
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
}

const DEFAULT_POLL_MS = 10_000;
const MAX_POLL_MS = 60_000;
const FETCH_TIMEOUT_MS = 8_000;

let currentData: SgsViewData | null = null;
let currentActions: Array<{ id: string; description: string }> = [];
let selectedCard: string | null = null;
let unbindPress: (() => void) | null = null;
let submitting = false;
let boot: Bootstrap | null = null;
/** 当前自动打开的 pending 浮层签名（空 = 没有 pending 浮层）。 */
let pendingOverlaySig = "";
/** 观星本地槽位（端侧草稿；提交时一次性发 arrange:<csv>）。 */
let gxSlots: GxSlots | null = null;
let gxInitial: GxSlots | null = null;
let gxSuppressClickUntil = 0;

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

function canPlaySelected(): boolean {
  return Boolean(selectedCard && currentActions.some((a) => a.id === `play:${selectedCard}`));
}

function syncSelection(): void {
  document.querySelectorAll<HTMLElement>(".sgs-hand .sgs-card").forEach((el) => {
    el.classList.toggle("selected", el.dataset.card === selectedCard);
  });
  const confirm = document.querySelector<HTMLButtonElement>('[data-op="confirm"]');
  if (confirm) confirm.disabled = !canPlaySelected();
  const cancel = document.querySelector<HTMLButtonElement>('[data-op="cancel"]');
  if (cancel) cancel.disabled = !selectedCard;
  updateOpsSummary();
}

function toggleSelect(cardId: string): void {
  selectedCard = selectedCard === cardId ? null : cardId;
  syncSelection();
}

// ---------------------------------------------------------------------------
// 动作提交（P3.3）：草稿（选中）→ 按 pending 逐条提交；失败保留草稿并提示
// ---------------------------------------------------------------------------

function message(text: string, warn = false): void {
  const node = document.getElementById("sgs-msg");
  if (!node) return;
  node.textContent = text;
  node.classList.toggle("warn", warn);
}

async function submitAction(action: string): Promise<void> {
  if (!boot || submitting || !action) return;
  submitting = true;
  document.body.classList.add("busy");
  try {
    const r = await fetch(`/tabletop_game/${encodeURIComponent(boot.gameId)}/action`, {
      method: "POST",
      headers: { "content-type": "application/json", accept: "application/json" },
      body: JSON.stringify({ token: boot.token, room_id: boot.roomId, action }),
      cache: "no-store",
    });
    const body = (await r.json().catch(() => ({}))) as { ok?: boolean; message?: string; error?: string };
    if (!r.ok) {
      message(body.error ?? `提交失败（${r.status}）`, true);
    } else {
      message(body.message ?? "已提交");
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
    return;
  }
  const sig = pendingSignature(p);
  if (sig === pendingOverlaySig) return;
  pendingOverlaySig = sig;
  const blocking = isBlockingPending(p.kind);
  let body = "";
  switch (kind) {
    case "wuxie":
      body = renderWuxieOverlay(p, board, lastNames);
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
    case "arrange": {
      const zone = currentData?.special_zones?.guanxing;
      if (!gxSlots) {
        gxSlots = { ...(zone?.slots as GxSlots) };
        if (!gxSlots || Object.keys(gxSlots).length === 0) {
          gxSlots = initialSlots((zone?.cards ?? []).map((c) => c.id));
        }
        gxInitial = { ...gxSlots };
      }
      body = renderGuanxingOverlay(gxSlots, zone?.cards ?? [], { rows: zone?.rows ?? GX_ROWS });
      break;
    }
  }
  openOverlay(pendingTitle(p.kind), body, {
    blocking,
    size: kind === "choose_many" || kind === "pick_card" || kind === "arrange" ? "wide" : "normal",
  });
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
  const kind = el.dataset.press;
  if (kind === "hand" && el.dataset.card) {
    toggleSelect(el.dataset.card);
    return;
  }
  if (kind === "general") {
    const seatIdx = Number(el.dataset.seat);
    const seat = currentData?.public_board.seats[seatIdx];
    if (seat) openOverlay("武将详情", renderGeneralDetail(seat, lastNames));
    return;
  }
  if (kind === "discard" && currentData) {
    openOverlay("弃牌堆", renderDiscardList(currentData.public_board));
    return;
  }
  if (kind === "card" && el.dataset.card && currentData) {
    const card = findCard(currentData, el.dataset.card);
    if (card) openOverlay(cardLabel(card), renderCardDetail(card));
  }
}

function onClick(ev: MouseEvent): void {
  const target = ev.target as HTMLElement | null;
  const opEl = target?.closest?.("[data-op]") as HTMLElement | null;
  if (opEl) {
    const op = opEl.dataset.op;
    if (op === "confirm") {
      if (selectedCard && currentActions.some((a) => a.id === `play:${selectedCard}`)) {
        void submitAction(`play:${selectedCard}`);
      } else {
        message("该牌当前不可打出", true);
      }
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
  // 长按先给反馈（ui-spec §5.8）；手牌/装备/判定/其他小标记都开详情。
  const kind = el.dataset.press;
  if ((kind === "hand" || kind === "card") && el.dataset.card && currentData) {
    const card = findCard(currentData, el.dataset.card);
    if (card) {
      ev.preventDefault();
      openOverlay(cardLabel(card), renderCardDetail(card));
    }
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
    const r = await fetch(
      `/tabletop_game/${encodeURIComponent(boot.gameId)}/state?token=${encodeURIComponent(boot.token)}`,
      { signal: ctrl.signal, headers: { accept: "application/json" }, cache: "no-store" },
    );
    if (!r.ok) throw new Error(`state ${r.status}`);
    return (await r.json()) as StateResponse;
  } finally {
    clearTimeout(timer);
  }
}

function applyState(res: StateResponse): void {
  const data = res.player?.data ?? res.public?.data;
  if (!data) return;
  currentData = data;
  currentActions = res.actions ?? [];
  lastNames = res.names;
  // 选中的牌可能已被打出/弃置：不再存在于手牌则清空。
  if (selectedCard && !(data.player_area?.hand ?? []).some((c) => c.id === selectedCard)) {
    selectedCard = null;
  }
  const app = q<HTMLElement>("#sgs-app");
  if (app) {
    app.innerHTML = renderSgsApp(data, res.names, currentActions);
    bindInteractions();
    syncSelection();
  }
  const board = data.public_board;
  const meta = document.querySelector(".sgs-topbar .meta");
  if (meta && board) meta.textContent = `第 ${board.round} 轮 · ${board.turn.player_id ?? "-"}`;
  syncPendingOverlay();
}

async function refreshNow(): Promise<void> {
  if (!boot) return;
  const res = await fetchState(boot);
  // 详情浮层打开时不打断；pending 浮层必须随提交结果更新（否则会停在旧的待办上）。
  if (!isOverlayOpen() || pendingOverlaySig) applyState(res);
  setText("sgs-warn", "");
  setText("sgs-updated", `更新于 ${hhmmss(new Date())}`);
}

function start(): void {
  const bootstrap = readBootstrap();
  if (!bootstrap) return;
  boot = bootstrap;
  initOverlay();
  bindInteractions();
  // 点击/拖拽委托只绑一次（容器元素不变，innerHTML 重渲染不影响）
  q<HTMLElement>("#sgs-app")?.addEventListener("click", onClick);
  initGuanxingDrag();
  q<HTMLElement>("#sgs-overlay")?.addEventListener("click", onClick);

  let pollMs = bootstrap.pollMs ?? DEFAULT_POLL_MS;
  let timer: number | undefined;

  const schedule = (): void => {
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
      pollMs = bootstrap.pollMs ?? DEFAULT_POLL_MS;
      setText("sgs-warn", "");
      setText("sgs-updated", `更新于 ${hhmmss(new Date())}`);
      setText("sgs-sync", "已同步");
    } catch {
      pollMs = Math.min(MAX_POLL_MS, pollMs * 2);
      setText("sgs-warn", "连接中断");
    } finally {
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
