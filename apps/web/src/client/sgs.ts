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
let selectedCard: string | null = null;
let unbindPress: (() => void) | null = null;

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

function syncSelection(): void {
  document.querySelectorAll<HTMLElement>(".sgs-hand .sgs-card").forEach((el) => {
    el.classList.toggle("selected", el.dataset.card === selectedCard);
  });
  updateOpsSummary();
}

function toggleSelect(cardId: string): void {
  selectedCard = selectedCard === cardId ? null : cardId;
  syncSelection();
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
  lastNames = res.names;
  // 选中的牌可能已被打出/弃置：不再存在于手牌则清空。
  if (selectedCard && !(data.player_area?.hand ?? []).some((c) => c.id === selectedCard)) {
    selectedCard = null;
  }
  const app = q<HTMLElement>("#sgs-app");
  if (app) {
    app.innerHTML = renderSgsApp(data, res.names);
    bindInteractions();
    syncSelection();
  }
  const board = data.public_board;
  const meta = document.querySelector(".sgs-topbar .meta");
  if (meta && board) meta.textContent = `第 ${board.round} 轮 · ${board.turn.player_id ?? "-"}`;
}

function boot(): void {
  const bootstrap = readBootstrap();
  if (!bootstrap) return;
  initOverlay();
  bindInteractions();

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
      // 浮层打开时不打断用户：只更新状态条；等关闭后的下一次轮询再重渲染。
      if (!isOverlayOpen()) applyState(res);
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
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}

// 供 P3.3 复用的内部能力（提交/刷新）
export { closeOverlay };
