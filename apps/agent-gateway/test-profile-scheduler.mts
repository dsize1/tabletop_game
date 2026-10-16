// 画像日批调度单测：nextRunAt（跨时区/跨天）+ startDailyScheduler 触发。
// 运行：npx tsx test-profile-scheduler.mts
import assert from "node:assert";
import { nextRunAt, startDailyScheduler } from "./src/profile/scheduler.js";

const TZ = "Asia/Shanghai"; // UTC+8，无夏令时

// 2026-02-14 03:00 +08 = 2026-02-13T19:00:00Z → 当日 04:00
assert.equal(
  new Date(nextRunAt(Date.parse("2026-02-13T19:00:00Z"), 4, TZ)).toISOString(),
  "2026-02-13T20:00:00.000Z",
  "03:00 → 当日 04:00",
);
// 2026-02-14 05:00 +08 = 2026-02-13T21:00:00Z → 次日 04:00
assert.equal(
  new Date(nextRunAt(Date.parse("2026-02-13T21:00:00Z"), 4, TZ)).toISOString(),
  "2026-02-14T20:00:00.000Z",
  "05:00 → 次日 04:00",
);
// 恰好 04:00 → 次日（严格大于 now）
assert.equal(
  new Date(nextRunAt(Date.parse("2026-02-13T20:00:00Z"), 4, TZ)).toISOString(),
  "2026-02-14T20:00:00.000Z",
  "04:00 整 → 次日",
);

// startDailyScheduler：注入时钟/定时器，触发 run 一次
{
  const at0300 = Date.parse("2026-02-13T19:00:00Z");
  let delay = -1;
  let cb: (() => void) | null = null;
  let fired = 0;
  const stop = startDailyScheduler({
    hour: 4,
    tz: TZ,
    now: () => at0300,
    setTimeoutFn: ((fn: () => void, ms: number) => {
      cb = fn;
      delay = ms;
      return 1 as unknown as ReturnType<typeof setTimeout>;
    }) as unknown as typeof setTimeout,
    clearTimeoutFn: (() => {}) as unknown as typeof clearTimeout,
    log: () => {},
    run: async () => {
      fired++;
    },
  });
  assert.equal(delay, 3600_000, "03:00 距 04:00 应为一小时");
  assert.ok(cb, "应已注册定时器");
  await (cb as unknown as () => Promise<void>)();
  assert.equal(fired, 1, "到点应触发 run");
  stop();
}

console.log("✅ profile-scheduler 单测全部通过");
