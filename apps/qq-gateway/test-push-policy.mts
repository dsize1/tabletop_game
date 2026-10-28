// qq.push 投递策略单测：纯函数 + 假时钟（不依赖网络/Redis）。
// 运行：npx tsx test-push-policy.mts（或仓库根 `npm run test:qq-gateway`）
import assert from "node:assert";
import {
  PASSIVE_REPLY_WINDOW_MS,
  describeFailure,
  entryAgeMs,
  entryTimestamp,
  isExpiredReply,
  isPermanentFailure,
} from "./src/push-policy.js";

let n = 0;
const t = (name: string, fn: () => void) => {
  fn();
  n += 1;
  console.log(`  ✓ ${name}`);
};

// ---- 永久失败判定 ----
t("40034024（msg_id 无效或越权）→ 永久", () => {
  assert.equal(
    isPermanentFailure({ httpStatus: 400, bizCode: 40034024, bizMessage: "请求参数msg_id无效或越权" }),
    true,
  );
});

t("11321（group_id 无效）→ 永久", () => {
  assert.equal(isPermanentFailure({ httpStatus: 404, bizCode: 11321 }), true);
});

t("401/403/404 等 4xx → 永久", () => {
  for (const httpStatus of [400, 401, 403, 404, 413]) {
    assert.equal(isPermanentFailure({ httpStatus }), true, `http=${httpStatus}`);
  }
});

t("5xx / 429 / 网络错误 → 可重试", () => {
  assert.equal(isPermanentFailure({ httpStatus: 500 }), false);
  assert.equal(isPermanentFailure({ httpStatus: 502 }), false);
  assert.equal(isPermanentFailure({ httpStatus: 429 }), false); // 频控，稍后重试
  assert.equal(isPermanentFailure(new Error("fetch failed")), false);
  assert.equal(isPermanentFailure(undefined), false);
});

// ---- Stream ID 时间戳 ----
t("entryTimestamp 解析 `<ms>-<seq>`", () => {
  assert.equal(entryTimestamp("1792211041088-0"), 1792211041088);
  assert.equal(entryTimestamp("1792211041088-17"), 1792211041088);
});

t("entryTimestamp 非法 id 视作极旧（0）", () => {
  assert.equal(entryTimestamp("not-a-stream-id"), 0);
  assert.equal(entryTimestamp(""), 0);
});

t("entryAgeMs 用假时钟计算滞留时长", () => {
  assert.equal(entryAgeMs("1000-0", 61_000), 60_000);
});

// ---- 超窗丢弃 ----
const NOW = 1_800_000_000_000;
const OLD_ID = `${NOW - PASSIVE_REPLY_WINDOW_MS - 1}-0`;
const FRESH_ID = `${NOW - 60_000}-0`;

t("被动回复超窗 → 丢弃", () => {
  assert.equal(isExpiredReply(OLD_ID, { reply_msg_id: "ROBOT1.0_x" }, NOW), true);
});

t("被动回复窗口内 → 保留发送", () => {
  assert.equal(isExpiredReply(FRESH_ID, { reply_msg_id: "ROBOT1.0_x" }, NOW), false);
});

t("恰好 5 分钟（边界）→ 仍可尝试", () => {
  const edge = `${NOW - PASSIVE_REPLY_WINDOW_MS}-0`;
  assert.equal(isExpiredReply(edge, { reply_msg_id: "ROBOT1.0_x" }, NOW), false);
});

t("主动消息（无 reply_msg_id）不受窗口约束", () => {
  assert.equal(isExpiredReply(OLD_ID, { reply_msg_id: null }, NOW), false);
  assert.equal(isExpiredReply(OLD_ID, {}, NOW), false);
});

// ---- 日志摘要 ----
t("describeFailure 输出单行摘要", () => {
  const s = describeFailure({ httpStatus: 400, bizCode: 40034024, bizMessage: "msg_id 无效" });
  assert.equal(s, "http=400 code=40034024 msg_id 无效");
  assert.equal(describeFailure(new Error("boom")), "boom");
});

console.log(`\n✅ push-policy：${n} 项断言全部通过`);
