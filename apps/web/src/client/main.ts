// 浏览器运行时：页内双模式切换 + 状态轮询 + 重新激活刷新。
//
// 由 esbuild 打包为 `/assets/app.js`；与 SSR 共用 `src/view/**` 的纯渲染函数。
// 安全：token 只从 bootstrap JSON 读，随 `/state` 请求传递，不落 localStorage。

import { buildBoardScene, phaseLabel } from "../view/board/scene.js";
import { renderBoardSvg } from "../view/board/svg.js";
import { renderGameTextBody } from "../view/text.js";
import type { GameView, ViewMode } from "../view/types.js";

interface Bootstrap {
  gameId: string;
  token: string;
  mode: ViewMode;
  pollMs?: number;
}

const DEFAULT_POLL_MS = 10_000;
const MAX_POLL_MS = 60_000;
const FETCH_TIMEOUT_MS = 8_000;

function readBootstrap(): Bootstrap | null {
  const node = document.getElementById("bootstrap");
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

function setMode(mode: ViewMode, opts: { updateUrl?: boolean } = {}): void {
  const main = q<HTMLElement>("#app");
  if (main) main.dataset.view = mode;
  document.querySelectorAll<HTMLButtonElement>(".seg button").forEach((b) => {
    b.setAttribute("aria-selected", String(b.dataset.mode === mode));
  });
  if (opts.updateUrl !== false) {
    const url = `${location.pathname}${location.search}#${mode}`;
    history.replaceState(null, "", url);
  }
}

function readModeFromHash(fallback: ViewMode): ViewMode {
  return location.hash === "#text" ? "text" : location.hash === "#board" ? "board" : fallback;
}

let lastPayload = "";

function render(view: GameView): void {
  // 数据没变就不动 DOM（排除 server_time 等易变字段，避免每 10s 无谓重建）
  const payload = JSON.stringify({ p: view.public, m: view.player, n: view.names });
  if (payload === lastPayload) return;

  let boardHtml: string;
  let textHtml: string;
  try {
    boardHtml = renderBoardSvg(buildBoardScene(view));
    textHtml = renderGameTextBody(view);
  } catch (err) {
    console.error("[web] 渲染失败，保留上一帧", err);
    return;
  }
  const board = q<HTMLElement>("#board-view");
  const text = q<HTMLElement>("#text-view");
  if (board) board.innerHTML = boardHtml;
  if (text) text.innerHTML = textHtml;
  lastPayload = payload;

  const d = view.public?.data ?? {};
  const round = q<HTMLElement>(".topbar .round");
  if (round) round.textContent = `第 ${d.round ?? "-"} 轮 · ${phaseLabel(d.phase)}`;
  const updated = q<HTMLElement>("#updated");
  if (updated) updated.textContent = `更新于 ${new Date().toLocaleTimeString("zh-CN", { hour12: false })}`;
}

function setConn(text: string, warn = false): void {
  const el = q<HTMLElement>("#conn");
  if (!el) return;
  el.textContent = text;
  el.classList.toggle("warn", warn);
}

function startPolling(boot: Bootstrap): void {
  const baseMs = Math.max(2_000, Number(boot.pollMs) || DEFAULT_POLL_MS);
  let delay = baseMs;
  let timer: number | undefined;
  let inflight = false;
  let failures = 0;

  const stateUrl = `/tabletop_game/${encodeURIComponent(boot.gameId)}/state` +
    `?token=${encodeURIComponent(boot.token)}`;

  const schedule = (ms: number): void => {
    if (timer !== undefined) window.clearTimeout(timer);
    timer = window.setTimeout(tick, ms);
  };

  const tick = async (): Promise<void> => {
    if (document.hidden) {
      setConn("后台暂停");
      schedule(baseMs);
      return;
    }
    if (inflight) return;
    inflight = true;
    const ctrl = new AbortController();
    const killer = window.setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
    try {
      const res = await fetch(stateUrl, {
        signal: ctrl.signal,
        headers: { accept: "application/json" },
        cache: "no-store",
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const view = (await res.json()) as GameView;
      render(view);
      failures = 0;
      delay = baseMs;
      setConn("已连接");
    } catch (err) {
      failures += 1;
      delay = Math.min(MAX_POLL_MS, baseMs * 2 ** Math.min(failures, 3));
      setConn(failures > 2 ? "连接中断，重试中…" : "同步失败，重试中…", true);
      if (failures > 2) console.warn("[web] /state 拉取失败", err);
    } finally {
      window.clearTimeout(killer);
      inflight = false;
      schedule(delay);
    }
  };

  schedule(baseMs);

  // 重新激活 / 从 bfcache 恢复 → 立即刷新（不等下一个轮询周期）
  const refreshNow = (): void => {
    if (document.hidden) return;
    if (timer !== undefined) window.clearTimeout(timer);
    void tick();
  };
  document.addEventListener("visibilitychange", () => {
    setConn(document.hidden ? "后台暂停" : "已连接");
    if (!document.hidden) refreshNow();
  });
  window.addEventListener("focus", refreshNow);
  window.addEventListener("pageshow", (e) => {
    if ((e as PageTransitionEvent).persisted) refreshNow();
  });
  window.addEventListener("online", refreshNow);
}

function main(): void {
  const boot = readBootstrap();
  if (!boot) return;

  setMode(readModeFromHash(boot.mode), { updateUrl: false });

  document.querySelectorAll<HTMLButtonElement>(".seg button").forEach((btn) => {
    btn.addEventListener("click", () => {
      const mode = (btn.dataset.mode === "text" ? "text" : "board") as ViewMode;
      setMode(mode);
    });
  });
  window.addEventListener("hashchange", () => {
    setMode(readModeFromHash(boot.mode), { updateUrl: false });
  });

  startPolling(boot);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", main);
} else {
  main();
}
