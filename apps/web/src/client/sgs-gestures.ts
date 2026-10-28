// 指针手势（P3.2）：短按 / 长按 / 位移取消 / pointercancel。
//
// 规格（ui-spec §5）：只用 Pointer Events；点击判定在 `up`；
// 短按 < 400ms 且位移 ≤ 8px；长按 ≥ 400ms（到时即反馈）；移动 > 8px 取消本次按击。
// `classifyPress` 是纯函数（可单测），`bindPress` 负责 DOM 绑定与定时器。

export const TAP_MAX_MS = 400;
export const LONG_PRESS_MS = 400;
export const MOVE_TOLERANCE_PX = 8;

export interface PressSample {
  downAt: number;
  upAt: number;
  dx: number;
  dy: number;
  cancelled?: boolean;
  /** 长按定时器是否已经触发（到时即反馈）。 */
  longFired?: boolean;
}

export type PressKind = "tap" | "longpress" | "move" | "cancel";

export function classifyPress(sample: PressSample): PressKind {
  if (sample.cancelled) return "cancel";
  const dist = Math.hypot(sample.dx, sample.dy);
  if (dist > MOVE_TOLERANCE_PX) return "move";
  if (sample.longFired) return "longpress";
  return sample.upAt - sample.downAt >= LONG_PRESS_MS ? "longpress" : "tap";
}

export interface PressHandlers {
  onTap?: (el: HTMLElement, ev: PointerEvent) => void;
  onLongPress?: (el: HTMLElement, ev: PointerEvent) => void;
  /** 每次按击结束（含取消）；可用于清理视觉态。 */
  onEnd?: (kind: PressKind, el: HTMLElement) => void;
}

interface ActivePress {
  el: HTMLElement;
  pointerId: number;
  x: number;
  y: number;
  at: number;
  longFired: boolean;
  moved: boolean;
  cancelled: boolean;
  timer: number | undefined;
}

/**
 * 在 `root` 上做事件委托：只有自身或祖先带 `data-press` 的元素参与手势。
 * 返回解绑函数（客户端重渲染后需重新绑定）。
 */
export function bindPress(root: HTMLElement, handlers: PressHandlers): () => void {
  let active: ActivePress | null = null;

  const clearTimer = (p: ActivePress | null): void => {
    if (p && p.timer !== undefined) {
      window.clearTimeout(p.timer);
      p.timer = undefined;
    }
  };

  const reset = (): void => {
    if (active) clearTimer(active);
    active = null;
  };

  const onDown = (ev: PointerEvent): void => {
    if (ev.pointerType === "mouse" && ev.button !== 0) return;
    const target = ev.target as HTMLElement | null;
    const el = target?.closest?.("[data-press]") as HTMLElement | null;
    if (!el || !root.contains(el)) return;
    reset();
    active = {
      el,
      pointerId: ev.pointerId,
      x: ev.clientX,
      y: ev.clientY,
      at: performance.now(),
      longFired: false,
      moved: false,
      cancelled: false,
      timer: undefined,
    };
    const current = active;
    current.timer = window.setTimeout(() => {
      if (!active || active !== current || current.moved || current.cancelled) return;
      current.longFired = true;
      current.el.classList.add("pressed");
      handlers.onLongPress?.(current.el, ev);
    }, LONG_PRESS_MS);
  };

  const onMove = (ev: PointerEvent): void => {
    if (!active || ev.pointerId !== active.pointerId) return;
    const dx = ev.clientX - active.x;
    const dy = ev.clientY - active.y;
    if (Math.hypot(dx, dy) > MOVE_TOLERANCE_PX) {
      active.moved = true;
      clearTimer(active);
      active.el.classList.remove("pressed");
    }
  };

  const onUp = (ev: PointerEvent): void => {
    const p = active;
    if (!p || ev.pointerId !== p.pointerId) return;
    active = null;
    clearTimer(p);
    p.el.classList.remove("pressed");
    const kind = classifyPress({
      downAt: p.at,
      upAt: performance.now(),
      dx: ev.clientX - p.x,
      dy: ev.clientY - p.y,
      cancelled: p.cancelled,
      longFired: p.longFired,
    });
    if (kind === "tap") handlers.onTap?.(p.el, ev);
    handlers.onEnd?.(kind, p.el);
  };

  const onCancel = (ev: PointerEvent): void => {
    const p = active;
    if (!p || ev.pointerId !== p.pointerId) return;
    active = null;
    clearTimer(p);
    p.el.classList.remove("pressed");
    handlers.onEnd?.("cancel", p.el);
  };

  const onScroll = (): void => {
    if (!active) return;
    // 滚动意图：立即结束本次按击（不产生点击/长按）。
    clearTimer(active);
    active.el.classList.remove("pressed");
    active = null;
  };

  root.addEventListener("pointerdown", onDown);
  root.addEventListener("pointermove", onMove);
  root.addEventListener("pointerup", onUp);
  root.addEventListener("pointercancel", onCancel);
  root.addEventListener("scroll", onScroll, true);

  return () => {
    reset();
    root.removeEventListener("pointerdown", onDown);
    root.removeEventListener("pointermove", onMove);
    root.removeEventListener("pointerup", onUp);
    root.removeEventListener("pointercancel", onCancel);
    root.removeEventListener("scroll", onScroll, true);
  };
}
