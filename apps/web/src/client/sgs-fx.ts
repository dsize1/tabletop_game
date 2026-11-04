// 对局特效层（客户端运行时，P0）：不改变 SSR/现有 DOM 结构，只追加 fixed 图层。
//
// 语义：
// - 队列来自 `view/sgs/fx.ts` 的历史差分 cue；首帧由调用方保证为空（不回放旧历史）；
// - 播放期间锁定输入（由 sgs.ts 暂停轮询/加 body class），结束后回调 drain → 刷新一次；
// - 无跳过按钮；超过 12 条由纯函数截断为最近 12 条；
// - prefers-reduced-motion → 只做描边/文字，不闪屏；
// - 图片加载失败时退化为 CSS 光圈（不出现裂图）。

import type { FxCue } from "../view/sgs/fx.js";
import { FX_ASSETS } from "../view/sgs/fx-manifest.js";

export interface FxOptions {
  /** 本人座位（用于定位 `.sgs-personal`）；未知返回 null。 */
  getSelfSeat: () => number | null;
  /** 进入/退出消费锁定；sgs.ts 负责暂停轮询与 body class。 */
  onLockChange: (locked: boolean) => void;
  /** 队列消费完成（刷新一次并恢复轮询）。 */
  onDrain: () => void;
}

const LAYER_ID = "sgs-fx-layer";
const STYLE_ID = "sgs-fx-style";
const MAX_TOTAL_MS = 3000;

let opts: FxOptions | null = null;
let layer: HTMLDivElement | null = null;
let queue: FxCue[] = [];
let running = false;
let locked = false;
let timer = 0;
let active = new Set<HTMLElement>();
let motionQuery: MediaQueryList | null = null;

const KIND_BASE_MS: Record<FxCue["kind"], number> = {
  slash: 340,
  dodge: 300,
  peach: 360,
  wine: 320,
  thunder: 420,
  fire: 380,
  chain: 300,
  heal: 300,
  lose: 280,
};

const ASSET_BY_KIND: Record<FxCue["kind"], string | null> = {
  slash: "slash",
  dodge: "dodge",
  peach: "peach",
  wine: "wine",
  thunder: "thunder_bolt",
  fire: "fire",
  chain: "chain",
  heal: "heal",
  lose: null,
};

function reducedMotion(): boolean {
  return motionQuery?.matches ?? false;
}

function injectStyle(): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement("style");
  style.id = STYLE_ID;
  style.textContent = `
#sgs-fx-layer{position:fixed;inset:0;z-index:95;overflow:hidden;pointer-events:none;}
#sgs-fx-layer .fx-cue{position:fixed;transform:translate(-50%,-50%);pointer-events:none;will-change:transform,opacity;}
#sgs-fx-layer .fx-img{display:block;width:100%;height:100%;object-fit:contain;opacity:0;
  animation:fx-appear var(--fx-dur,320ms) ease-out both;mix-blend-mode:screen;}
/* 动图本身负责运动；这里只做淡入淡出（避免 CSS 再叠加旋转/缩放）。 */
#sgs-fx-layer .fx-ring{position:absolute;inset:-6px;border-radius:12px;border:2px solid rgba(227,179,65,.75);
  box-shadow:0 0 18px rgba(227,179,65,.45);opacity:0;animation:fx-ring var(--fx-dur,320ms) ease-out both;}
#sgs-fx-layer .fx-caption{position:absolute;left:50%;top:-10px;transform:translate(-50%,-100%);
  white-space:nowrap;padding:3px 8px;border-radius:999px;background:rgba(10,14,20,.82);
  border:1px solid rgba(227,179,65,.35);color:#f2e6c8;font-size:11px;line-height:1.2;
  opacity:0;animation:fx-caption var(--fx-dur,320ms) ease-out both;}
#sgs-fx-layer .fx-flash{position:fixed;inset:0;background:
  radial-gradient(circle at 50% 45%, rgba(210,235,255,.85), rgba(90,130,255,.25) 35%, transparent 70%);
  opacity:0;animation:fx-flash 420ms ease-out both;mix-blend-mode:screen;pointer-events:none;}
#sgs-fx-layer .fx-flash-img{position:fixed;inset:0;width:100%;height:100%;object-fit:cover;
  opacity:0;animation:fx-flash 420ms ease-out both;mix-blend-mode:screen;pointer-events:none;}
#sgs-fx-layer .fx-reduced{width:100%;height:100%;display:flex;align-items:center;justify-content:center;
  border-radius:12px;border:2px solid rgba(227,179,65,.85);box-shadow:0 0 16px rgba(227,179,65,.45);
  color:#f5e7c4;font-size:12px;background:rgba(20,26,34,.35);}
body.sgs-fx-locked #sgs-app{pointer-events:none;cursor:progress;}
.fx-hit-target{animation:fx-target-glow 240ms ease-out both;}
@keyframes fx-target-glow{0%{filter:brightness(1);}20%{filter:brightness(1.55) saturate(1.2);}
  100%{filter:brightness(1);}}
body.sgs-fx-locked .sgs-hand,body.sgs-fx-locked .sgs-ops{filter:saturate(.75);}
@keyframes fx-appear{0%{opacity:0;}12%{opacity:1;}82%{opacity:1;}100%{opacity:0;}}
@keyframes fx-ring{0%{opacity:0;transform:scale(.92);}25%{opacity:1;}100%{opacity:0;transform:scale(1.08);}}
@keyframes fx-caption{0%{opacity:0;transform:translate(-50%,-80%);}20%{opacity:1;}80%{opacity:1;}100%{opacity:0;transform:translate(-50%,-120%);}}
@keyframes fx-flash{0%,100%{opacity:0;}10%{opacity:.34;}20%{opacity:0;}30%{opacity:.26;}45%{opacity:0;}}
@media (prefers-reduced-motion: reduce){
  #sgs-fx-layer .fx-img,#sgs-fx-layer .fx-ring,#sgs-fx-layer .fx-caption,#sgs-fx-layer .fx-flash{animation:none!important;}
}`;
  document.head.appendChild(style);
}

function ensureLayer(): HTMLDivElement {
  if (layer?.isConnected) return layer;
  injectStyle();
  const el = document.createElement("div");
  el.id = LAYER_ID;
  el.setAttribute("aria-hidden", "true");
  document.body.appendChild(el);
  layer = el;
  return el;
}

function setLocked(next: boolean): void {
  if (locked === next) return;
  locked = next;
  opts?.onLockChange(next);
}

function clearTimer(): void {
  if (timer) window.clearTimeout(timer);
  timer = 0;
}

function removeActive(): void {
  for (const el of active) el.remove();
  active.clear();
  document.querySelectorAll(".fx-hit-target").forEach((el) => el.classList.remove("fx-hit-target"));
}

function targetRect(seat: number | null): DOMRect | null {
  if (seat === null) return null;
  const self = opts?.getSelfSeat() ?? null;
  if (seat === self) {
    const avatar = document.querySelector<HTMLElement>(".sgs-personal .sgs-me-portrait");
    const personal = document.querySelector<HTMLElement>(".sgs-personal");
    return (avatar ?? personal)?.getBoundingClientRect() ?? null;
  }
  return (
    document.querySelector<HTMLElement>(`.sgs-opponent[data-seat="${seat}"]`)?.getBoundingClientRect() ??
    null
  );
}

function captionFor(cue: FxCue): string {
  if (cue.text) return cue.text;
  if (cue.kind === "heal") return `回复 ${cue.amount ?? 1} 点体力`;
  if (cue.kind === "lose") return `失去 ${cue.amount ?? 1} 点体力`;
  if (cue.amount) return `${cue.kind} ${cue.amount}`;
  return cue.kind;
}

function playCue(cue: FxCue, duration: number): void {
  const root = ensureLayer();
  const rect = targetRect(cue.seat);
  const centerX = rect ? rect.left + rect.width / 2 : window.innerWidth / 2;
  const centerY = rect ? rect.top + rect.height / 2 : window.innerHeight / 2;
  const size = rect ? Math.min(Math.max(rect.width * 1.7, 130), 260) : 220;
  const half = size / 2;
  const left = rect ? Math.min(Math.max(centerX, half + 6), window.innerWidth - half - 6) : centerX;
  const top = rect ? Math.min(Math.max(centerY, half + 6), window.innerHeight - half - 6) : centerY;
  const cueEl = document.createElement("div");
  cueEl.className = `fx-cue fx-${cue.kind}`;
  cueEl.style.left = `${left}px`;
  cueEl.style.top = `${top}px`;
  cueEl.style.width = `${size}px`;
  cueEl.style.height = `${size}px`;
  cueEl.style.setProperty("--fx-dur", `${duration}ms`);

  if (rect) {
    const target = document.querySelector<HTMLElement>(
      cue.seat === (opts?.getSelfSeat() ?? null)
        ? ".sgs-personal"
        : `.sgs-opponent[data-seat="${cue.seat}"]`,
    );
    if (target) {
      target.classList.add("fx-hit-target");
      window.setTimeout(() => target.classList.remove("fx-hit-target"), duration);
    }
  }

  if (reducedMotion()) {
    const reduced = document.createElement("div");
    reduced.className = "fx-reduced";
    reduced.textContent = captionFor(cue);
    cueEl.appendChild(reduced);
    root.appendChild(cueEl);
    active.add(cueEl);
    return;
  }

  const assetId = ASSET_BY_KIND[cue.kind];
  const asset = assetId ? FX_ASSETS[assetId] : undefined;
  if (asset) {
    const img = document.createElement("img");
    img.className = "fx-img";
    img.alt = "";
    img.src = asset.url;
    if (asset.blend === "normal") img.style.mixBlendMode = "normal";
    img.addEventListener("error", () => {
      img.remove();
      if (!cueEl.querySelector(".fx-ring")) {
        const ring = document.createElement("div");
        ring.className = "fx-ring";
        cueEl.appendChild(ring);
      }
    });
    cueEl.appendChild(img);
  }
  if (!cueEl.querySelector(".fx-ring")) {
    const ring = document.createElement("div");
    ring.className = "fx-ring";
    cueEl.appendChild(ring);
  }

  const caption = document.createElement("div");
  caption.className = "fx-caption";
  caption.textContent = `${captionFor(cue)}${cue.count > 1 ? ` ×${cue.count}` : ""}`;
  cueEl.appendChild(caption);

  if (cue.kind === "thunder" && cue.variant === "big") {
    const flashAsset = FX_ASSETS["thunder_flash"];
    const flash = document.createElement(flashAsset ? "img" : "div");
    flash.className = flashAsset ? "fx-flash-img" : "fx-flash";
    if (flashAsset && flash instanceof HTMLImageElement) {
      flash.src = flashAsset.url;
      flash.alt = "";
      flash.addEventListener("error", () => flash.remove());
    }
    root.appendChild(flash);
    window.setTimeout(() => flash.remove(), duration + 40);
  }

  root.appendChild(cueEl);
  active.add(cueEl);
  window.setTimeout(() => {
    cueEl.remove();
    active.delete(cueEl);
  }, duration + 40);
}

function nextDelay(cues: FxCue[], index: number, selfSeat: number | null): number {
  const cue = cues[index];
  if (!cue) return 0;
  const own = selfSeat !== null && cue.seat === selfSeat;
  const assetId = ASSET_BY_KIND[cue.kind];
  const asset = assetId ? FX_ASSETS[assetId] : undefined;
  const animMs = asset ? asset.frames * asset.duration + 40 : 0;
  const base = Math.max(
    own ? Math.min(220, KIND_BASE_MS[cue.kind]) : KIND_BASE_MS[cue.kind],
    // 动图至少播完一遍（5×80ms）再切下一条；自身动作也不能少于 200ms。
    animMs,
    own ? 200 : 0,
  );
  const dense = cues.length > 6 ? 0.72 : 1;
  const merged = cue.count > 1 ? 1.12 : 1;
  return Math.max(150, Math.round(base * dense * merged));
}

function totalBudget(cues: FxCue[], selfSeat: number | null): number[] {
  const durations = cues.map((_, i) => nextDelay(cues, i, selfSeat));
  const sum = durations.reduce((a, b) => a + b, 0);
  if (sum <= MAX_TOTAL_MS) return durations;
  const scale = MAX_TOTAL_MS / sum;
  return durations.map((d) => Math.max(120, Math.round(d * scale)));
}

function drain(): void {
  if (running || queue.length === 0) {
    if (locked) setLocked(false);
    return;
  }
  running = true;
  setLocked(true);
  const selfSeat = opts?.getSelfSeat() ?? null;
  const cues = queue.splice(0, queue.length).slice(-12);
  const durations = totalBudget(cues, selfSeat);
  // reduced-motion：总锁定时间也被压到 ≤300ms（plan）。
  const reduced = reducedMotion();
  const durationsReduced = cues.map(() => Math.max(80, Math.floor(300 / Math.max(1, cues.length))));

  const step = (i: number): void => {
    if (i >= cues.length) {
      running = false;
      if (queue.length > 0) {
        drain();
        return;
      }
      setLocked(false);
      opts?.onDrain();
      return;
    }
    const d = reduced ? durationsReduced[i] : durations[i];
    playCue(cues[i], d);
    timer = window.setTimeout(() => step(i + 1), d);
  };
  step(0);
}

export function initFx(options: FxOptions): void {
  opts = options;
  motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  ensureLayer();
  // 空闲预加载（不阻塞首屏）；失败静默。
  const preload = (): void => {
    for (const meta of Object.values(FX_ASSETS)) {
      const img = new Image();
      img.src = meta.url;
    }
  };
  const ric = (window as unknown as { requestIdleCallback?: (cb: () => void) => void }).requestIdleCallback;
  if (ric) ric(preload);
  else window.setTimeout(preload, 1500);
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) resetFx(true);
  });
}

export function enqueueFx(cues: FxCue[]): void {
  if (!cues.length) return;
  if (!layer) ensureLayer();
  const overlay = document.getElementById("sgs-overlay");
  if (document.hidden || overlay?.classList.contains("on")) return;
  queue.push(...cues);
  drain();
}

export function fxLocked(): boolean {
  return locked;
}

/** 清空队列/节点；`unlock=true` 时恢复输入（隐藏/卸载用，不触发 drain 刷新）。 */
export function resetFx(unlock = true): void {
  queue = [];
  running = false;
  clearTimer();
  removeActive();
  if (unlock) setLocked(false);
}
