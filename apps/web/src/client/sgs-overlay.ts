// 居中浮层管理器（P3.2）：单层、遮罩、滚动锁、Esc / 浏览器返回 / 关闭按钮、焦点还原。

import { renderOverlayShell } from "../view/sgs/overlay.js";

let opened = false;
let blocking = false;
let lastFocus: Element | null = null;

function root(): HTMLElement | null {
  return document.getElementById("sgs-overlay");
}

export function isOverlayOpen(): boolean {
  return opened;
}

/** 打开（或替换）浮层内容；同一时刻只有一层。 */
export function openOverlay(
  title: string,
  bodyHtml: string,
  opts: { size?: "normal" | "wide"; blocking?: boolean } = {},
): void {
  const el = root();
  if (!el) return;
  blocking = Boolean(opts.blocking);
  if (!opened) {
    lastFocus = document.activeElement;
    document.body.style.overflow = "hidden";
    // 让「安卓返回」先关浮层而不是离开页面
    history.pushState({ sgsOverlay: true }, "", location.href);
    opened = true;
  }
  el.innerHTML = renderOverlayShell(title, bodyHtml, { size: opts.size, blocking });
  el.classList.add("on");
  el.setAttribute("aria-hidden", "false");
  el.classList.toggle("blocking", blocking);
  const focusTarget =
    el.querySelector<HTMLElement>("[data-overlay-close='button']") ??
    el.querySelector<HTMLElement>("[data-action]") ??
    el.querySelector<HTMLElement>(".sgs-overlay-card");
  focusTarget?.focus();
}

export function closeOverlay(force = false): void {
  if (!opened) return;
  if (blocking && !force) return; // 必须显式选择：忽略关注意图
  opened = false;
  const el = root();
  if (el) {
    el.classList.remove("on");
    el.setAttribute("aria-hidden", "true");
    el.innerHTML = "";
  }
  document.body.style.overflow = "";
  if (lastFocus && lastFocus instanceof HTMLElement) lastFocus.focus();
  lastFocus = null;
  if (history.state && (history.state as { sgsOverlay?: boolean }).sgsOverlay) {
    history.back();
  }
}

/** 只初始化一次：遮罩/关闭按钮委托、Esc、浏览器返回。 */
export function initOverlay(): void {
  const el = root();
  if (!el) return;
  el.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement | null;
    if (!target?.closest?.("[data-overlay-close]")) return;
    // 阻塞型浮层没有关闭按钮/遮罩标记，这里再兜一层
    if (!blocking) closeOverlay();
  });
  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" && opened && !blocking) {
      ev.preventDefault();
      closeOverlay();
    }
  });
  window.addEventListener("popstate", () => {
    // 用户按了浏览器/安卓返回：只关浮层，不执行 closeOverlay 里的 history.back()
    if (!opened) return;
    opened = false;
    const node = root();
    if (node) {
      node.classList.remove("on");
      node.setAttribute("aria-hidden", "true");
      node.innerHTML = "";
    }
    document.body.style.overflow = "";
    if (lastFocus && lastFocus instanceof HTMLElement) lastFocus.focus();
    lastFocus = null;
  });
}
