//! 日批调度：每日 `CHAT_TZ` 的 `hour:00` 触发（自维护定时器，无新依赖）。
//! 时钟与定时器可注入，便于离线单测「4 点触发」。

import { CHAT_TZ } from "../time-context.js";

function parts(epochMs: number, tz: string) {
  const p = Object.fromEntries(
    new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      hour12: false,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    })
      .formatToParts(new Date(epochMs))
      .map((x) => [x.type, x.value]),
  );
  return {
    year: Number(p.year),
    month: Number(p.month),
    day: Number(p.day),
    hour: Number(p.hour) % 24,
    minute: Number(p.minute),
    second: Number(p.second),
  };
}

/** 目标时区相对 UTC 的偏移（ms）。 */
function offsetMs(epochMs: number, tz: string): number {
  const p = parts(epochMs, tz);
  const asUtc = Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second);
  return asUtc - epochMs;
}

/** `now` 之后最近一次 `hour:00`（给定时区）的 epoch ms。纯函数。 */
export function nextRunAt(now: number, hour: number, tz: string = CHAT_TZ): number {
  for (let addDays = 0; addDays <= 2; addDays++) {
    const probe = now + addDays * 86_400_000;
    const p = parts(probe, tz);
    const cand = Date.UTC(p.year, p.month - 1, p.day, hour, 0, 0) - offsetMs(probe, tz);
    if (cand > now) return cand;
  }
  return now + 86_400_000;
}

export interface DailySchedulerOptions {
  hour: number;
  tz?: string;
  run: () => Promise<void>;
  now?: () => number;
  setTimeoutFn?: typeof setTimeout;
  clearTimeoutFn?: typeof clearTimeout;
  log?: (msg: string) => void;
}

/** 启动每日调度；返回停止函数。 */
export function startDailyScheduler(opts: DailySchedulerOptions): () => void {
  const tz = opts.tz ?? CHAT_TZ;
  const now = opts.now ?? Date.now;
  const setT = opts.setTimeoutFn ?? setTimeout;
  const clearT = opts.clearTimeoutFn ?? clearTimeout;
  const log = opts.log ?? ((m: string) => console.log(m));
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const schedule = () => {
    if (stopped) return;
    const t = now();
    const delay = Math.max(0, nextRunAt(t, opts.hour, tz) - t);
    log(
      `[profile] 下次蒸馏：${new Date(t + delay).toISOString()}（约 ${Math.round(delay / 60000)} 分钟后）`,
    );
    timer = setT(async () => {
      try {
        await opts.run();
      } catch (err) {
        console.error("[profile] 蒸馏任务失败:", err);
      }
      schedule();
    }, delay);
  };
  schedule();

  return () => {
    stopped = true;
    if (timer) clearT(timer);
  };
}
