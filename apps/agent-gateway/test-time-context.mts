// 时间上下文单测：纯函数 + 假时钟（不依赖网络/Redis）。
// 运行：npx tsx test-time-context.mts
import assert from "node:assert";
import {
  CHAT_TZ,
  CLUSTER_GAP_MS,
  TOPIC_BOUNDARY_MARK,
  chatGapMs,
  formatGap,
  formatTimeBlock,
  isStaleGap,
  lastTurnTs,
  lastUserContent,
  nowInTz,
  renderHistory,
  type TimedTurn,
} from "./src/time-context.js";
import { buildChatMessages } from "./src/chat.js";

const TZ = "Asia/Shanghai";
// 2026-02-14 09:12 (+08:00)
const NOW = Date.parse("2026-02-14T01:12:00Z");
const HOUR = 3600_000;
const MIN = 60_000;

// —— 时区快照 ——
{
  const s = nowInTz(NOW, TZ);
  assert.equal(s.date, "2026-02-14");
  assert.equal(s.weekday, "周六");
  assert.equal(s.hhmm, "09:12");
  assert.equal(s.mmdd, "02-14");
  assert.equal(s.tzLabel, "UTC+8");
}
// —— 跨 UTC 日界：18:00Z 属次日凌晨（+08:00）——
{
  const s = nowInTz(Date.parse("2026-02-13T18:00:00Z"), TZ);
  assert.equal(s.date, "2026-02-14");
  assert.equal(s.hhmm, "02:00");
}
// —— 半小时时区偏移标签 ——
assert.equal(nowInTz(NOW, "Asia/Kolkata").tzLabel, "UTC+5:30");

// —— 默认时区单点（容器默认 UTC，故显式固定）——
assert.equal(CHAT_TZ, "Asia/Shanghai");

// —— formatGap 边界（纯数字，不含时段语义）——
assert.equal(formatGap(0), "刚刚");
assert.equal(formatGap(MIN - 1), "刚刚");
assert.equal(formatGap(MIN), "1m");
assert.equal(formatGap(32 * MIN), "32m");
assert.equal(formatGap(HOUR - 1), "59m");
assert.equal(formatGap(HOUR), "1h");
assert.equal(formatGap(14 * HOUR + 32 * MIN), "14h32m");
assert.equal(formatGap(24 * HOUR - 1), "23h59m");
assert.equal(formatGap(24 * HOUR), "1d");
assert.equal(formatGap(24 * HOUR + 14 * HOUR), "1d14h");
assert.equal(formatGap(48 * HOUR), "2d");
assert.equal(formatGap(Number.NaN), "刚刚"); // 脏数据降级

// —— 时间块（独立 system 条目内容）——
assert.equal(
  formatTimeBlock(NOW, NOW - (14 * HOUR + 32 * MIN), TZ),
  "[时间] 现在 2026-02-14 周六 09:12 (UTC+8)；距上次对话 14h32m",
);
assert.equal(formatTimeBlock(NOW, undefined, TZ), "[时间] 现在 2026-02-14 周六 09:12 (UTC+8)");
// 旧数据无 ts → 只给「现在」，不编造间隔
assert.equal(formatTimeBlock(NOW, Number.NaN, TZ), "[时间] 现在 2026-02-14 周六 09:12 (UTC+8)");

// —— renderHistory：簇首锚点 / 簇内不重复 / 边界标记 ——
{
  const turns: TimedTurn[] = [
    { role: "user", content: "今天先到这", ts: NOW - (14 * HOUR + 32 * MIN) },
    { role: "assistant", content: "好，晚安", ts: NOW - (14 * HOUR + 31 * MIN) },
    { role: "user", content: "早", ts: NOW, boundary: true },
  ];
  const out = renderHistory(turns, { tz: TZ });
  // 首条打锚点（02-13 18:40）
  assert.equal(out[0]!.content, "[02-13 18:40] 今天先到这");
  // 簇内（1 分钟）不打锚点
  assert.equal(out[1]!.content, "好，晚安");
  // 跨簇 + 边界：先边界标记，再时间锚点
  assert.equal(out[2]!.content, `${TOPIC_BOUNDARY_MARK}\n[02-14 09:12] 早`);
  // 原始 content 未被改写
  assert.equal(turns[0]!.content, "今天先到这");
  assert.equal(turns[2]!.content, "早");
  assert.equal(out[1]!.role, "assistant");
  assert.equal(out.length, 3);
}
// —— 簇间隔阈值（左闭右开：≥ CLUSTER_GAP_MS 才另起簇）——
{
  const a: TimedTurn = { role: "user", content: "a", ts: NOW };
  const b: TimedTurn = { role: "user", content: "b", ts: NOW + CLUSTER_GAP_MS };
  const out = renderHistory([a, b], { tz: TZ });
  assert.equal(out[1]!.content.startsWith("["), true);
}
// —— 旧格式（无 ts）：不抛错、不打锚点，且不阻断后续锚点 ——
{
  const out = renderHistory(
    [
      { role: "user", content: "旧一" },
      { role: "assistant", content: "旧二" },
      { role: "user", content: "新", ts: NOW },
    ],
    { tz: TZ },
  );
  assert.equal(out[0]!.content, "旧一");
  assert.equal(out[1]!.content, "旧二");
  assert.equal(out[2]!.content, "[02-14 09:12] 新");
}
assert.deepEqual(renderHistory([]), []);

// —— 时间戳取用 ——
{
  const turns: TimedTurn[] = [
    { role: "user", content: "u1", ts: NOW - HOUR },
    { role: "assistant", content: "a1" },
    { role: "user", content: "u2", ts: NOW },
  ];
  assert.equal(lastTurnTs(turns), NOW);
  assert.equal(lastTurnTs([{ role: "user", content: "x" }]), undefined);
  assert.equal(chatGapMs(turns, NOW + 10 * MIN), 10 * MIN);
  assert.equal(chatGapMs([{ role: "user", content: "x" }], NOW), undefined);
  // 时钟回拨 → 夹到 0，不产出负间隔
  assert.equal(chatGapMs([{ role: "user", content: "x", ts: NOW + HOUR }], NOW), 0);
  assert.equal(lastUserContent(turns), "u2");
  assert.equal(lastUserContent([{ role: "assistant", content: "a" }]), undefined);
}

// —— 话题门控 ——
{
  const GATE = 30 * MIN;
  assert.equal(isStaleGap(undefined, GATE), false);
  assert.equal(isStaleGap(29 * MIN, GATE), false);
  assert.equal(isStaleGap(GATE, GATE), true); // 左闭
  assert.equal(isStaleGap(4 * HOUR, GATE), true);
  assert.equal(isStaleGap(4 * HOUR, 0), false); // 门控关闭
  assert.equal(isStaleGap(4 * HOUR, Number.NaN), false);
}

// —— buildChatMessages：system → 历史（带锚点）→ 时间块 → 当前消息 ——
{
  const history: TimedTurn[] = [{ role: "user", content: "今天先到这", ts: NOW - (14 * HOUR + 32 * MIN) }];
  const msgs = buildChatMessages("早", history, { now: NOW, tz: TZ });
  assert.equal(msgs.length, 4);
  assert.equal(msgs[0]!.role, "system");
  assert.ok(msgs[0]!.content.includes("不要"), "系统提示需约束「不主动报时间」");
  assert.equal(msgs[1]!.role, "user");
  assert.equal(msgs[1]!.content, "[02-13 18:40] 今天先到这");
  assert.equal(msgs[2]!.role, "system");
  assert.equal(msgs[2]!.content, "[时间] 现在 2026-02-14 周六 09:12 (UTC+8)；距上次对话 14h32m");
  // 当前消息原样（时间不夹带在用户消息里）
  assert.deepEqual(msgs[3], { role: "user", content: "早" });
  // 时间块是独立条目 → 出站回复里不会出现时间文本
  assert.equal(history[0]!.content, "今天先到这");
}

console.log("✅ time-context 单测全部通过");
