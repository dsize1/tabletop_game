// 时间相关护栏（harness）：
// 1) 源码不得依赖宿主本地时区做日历计算（容器默认 UTC 是已知隐患）；
// 2) 时间上下文相关配置的默认值必须与文档/实现一致。
// 运行：npx tsx test-time-guards.mts
import assert from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { CHAT_TZ, CLUSTER_GAP_MS, formatGap, formatTimeBlock, nowInTz } from "./src/time-context.js";
import { buildChatMessages } from "./src/chat.js";
import { assertNoInternalMarkers } from "./test-helpers.mts";

const srcDir = join(dirname(fileURLToPath(import.meta.url)), "src");

// ==================== 1) 禁止宿主本地时区的日历 API ====================
// 只允许 UTC 系列（getUTCDay 等）与 Intl + 显式 timeZone；本地时区取值/格式化一律禁止。
const BANNED = [
  ["getHours", /\.getHours\s*\(/],
  ["getMinutes", /\.getMinutes\s*\(/],
  ["getDate", /\.getDate\s*\(/],
  ["getDay", /\.getDay\s*\(/],
  ["getMonth", /\.getMonth\s*\(/],
  ["getFullYear", /\.getFullYear\s*\(/],
  ["toLocaleString", /\.toLocaleString\s*\(/],
  ["toLocaleDateString", /\.toLocaleDateString\s*\(/],
  ["toLocaleTimeString", /\.toLocaleTimeString\s*\(/],
] as const;

const violations: string[] = [];
for (const file of readdirSync(srcDir).filter((f) => f.endsWith(".ts"))) {
  const text = readFileSync(join(srcDir, file), "utf8");
  for (const [name, re] of BANNED) {
    if (re.test(text)) violations.push(`${file}: ${name}`);
  }
}
assert.deepEqual(violations, [], `不得依赖宿主本地时区：${violations.join(", ")}`);

// ==================== 2) 时区单一来源：显式 timeZone 才算数 ====================
{
  // UTC 与 +08:00 必须给出不同的「日期/时刻」——若错用宿主时区则会得到同一串
  const utc = nowInTz(Date.parse("2026-02-13T18:00:00Z"), "UTC");
  const sh = nowInTz(Date.parse("2026-02-13T18:00:00Z"), "Asia/Shanghai");
  assert.equal(utc.date, "2026-02-13");
  assert.equal(utc.hhmm, "18:00");
  assert.equal(sh.date, "2026-02-14");
  assert.equal(sh.hhmm, "02:00");
  assert.equal(sh.tzLabel, "UTC+8");
}

// ==================== 3) 间隔格式覆盖全量档位（防止改坏）====================
{
  const cases: Array<[number, string]> = [
    [0, "刚刚"],
    [59_999, "刚刚"],
    [60_000, "1m"],
    [59 * 60_000, "59m"],
    [60 * 60_000, "1h"],
    [24 * 3600_000 - 1, "23h59m"],
    [24 * 3600_000, "1d"],
    [24 * 3600_000 * 365, "365d"],
  ];
  for (const [ms, want] of cases) assert.equal(formatGap(ms), want, `formatGap(${ms})`);
}

// ==================== 4) 出站守卫：时间/边界绝不进出站消息 ====================
{
  const history = [{ role: "user" as const, content: "今天先到这", ts: Date.parse("2026-02-13T10:40:00Z") }];
  // 上下文（内部）里必须有时间标记
  const msgs = buildChatMessages("早", history, { now: Date.parse("2026-02-14T01:12:00Z") });
  assert.ok(msgs.some((m) => m.content.includes("[时间]")), "上下文应含时间块");
  // 出站内容不得含任何内部标记
  assertNoInternalMarkers("好，早上好呀", "模拟回复");
  assert.doesNotThrow(() => assertNoInternalMarkers("我们继续昨天那局吧", "普通回复"));
  // 自检：静态块本身必须被守卫识别（避免守卫形同虚设）
  assert.throws(() => assertNoInternalMarkers(formatTimeBlock(0, undefined), "时间块"));
  assert.throws(() => assertNoInternalMarkers("--- 话题切换 ---\n早", "边界标记"));
}

// ==================== 5) 配置默认值与文档一致 ====================
assert.equal(CHAT_TZ, "Asia/Shanghai");
assert.equal(CLUSTER_GAP_MS, 5 * 60_000);
{
  const env = readFileSync(join(srcDir, "..", ".env.example"), "utf8");
  for (const key of ["CHAT_TZ=Asia/Shanghai", "TOPIC_GAP_MINUTES=30", "TOPIC_SWITCH_THRESHOLD=0.7"]) {
    assert.ok(env.includes(key), `.env.example 缺少 ${key}`);
  }
}

console.log("✅ time-guards 护栏全部通过");
