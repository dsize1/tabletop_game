// 三国杀浏览器运行时（P3）：轮询刷新 + 状态条。
//
// 安全：token 只从页面 bootstrap JSON 读取，随 `/state` 请求传递，不落 localStorage。
// 交互（出牌/特殊区/观星）在 P3.3–P3.5 接线；本文件 P3.1 只负责「同一套纯函数重渲染」。

import { renderSgsApp } from "../view/sgs/page.js";
import type { SgsViewData } from "../view/sgs/types.js";

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

interface StateResponse {
  public?: { data?: SgsViewData };
  player?: { data?: SgsViewData };
  names?: Record<string, string>;
  error?: string;
  server_time?: string;
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
  const app = q<HTMLElement>("#sgs-app");
  if (app) app.innerHTML = renderSgsApp(data, res.names);
  const board = data.public_board;
  const meta = document.querySelector(".sgs-topbar .meta");
  if (meta && board) {
    meta.textContent = `第 ${board.round} 轮 · ${board.turn.player_id ?? "-"}`;
  }
}

function boot(): void {
  const bootstrap = readBootstrap();
  if (!bootstrap) return;
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
      applyState(res);
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

  // 首屏已 SSR：只更新状态条，然后进入轮询。
  setText("sgs-updated", `更新于 ${hhmmss(new Date())}`);
  schedule();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot);
} else {
  boot();
}
