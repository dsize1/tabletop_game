// 特效 cue / 轮询策略纯函数单测（无需起服务）。
// 运行：cd apps/web && npx tsx test-fx.mts
import assert from "node:assert";
import { awaitingLocal, nextPollDelay, stateSignature, sideSignature, BASE_POLL_MS, AWAIT_POLL_MS } from "./src/view/sgs/poll.js";
import { buildFxCues, historyLastSeq, type FxCue } from "./src/view/sgs/fx.js";
import type { SgsHistoryEntry, SgsSeat, SgsViewData } from "./src/view/sgs/types.js";

const seats = [
  { seat: 0, player_id: "p0", general_name: "刘备", hp: 4, max_hp: 4 },
  { seat: 1, player_id: "p1", general_name: "曹操", hp: 4, max_hp: 4 },
  { seat: 2, player_id: "p2", general_name: "孙权", hp: 3, max_hp: 3 },
] as unknown as SgsSeat[];

function hist(seq: number, text: string, actor?: string): SgsHistoryEntry {
  return { seq, turn: 1, actor: actor ?? null, text };
}

function view(history: SgsHistoryEntry[], seatStates = seats): SgsViewData {
  return {
    schema_version: 2,
    game: "sanguosha",
    phase: "turn",
    public_board: { seats: seatStates, history },
    player_area: null,
    special_zones: {},
  } as unknown as SgsViewData;
}

function kinds(cues: FxCue[]): string[] {
  return cues.map((c) => `${c.kind}@${c.seat}${c.count > 1 ? `×${c.count}` : ""}`);
}

// —— 轮询策略 ——
const base = view([]);
assert.equal(awaitingLocal(base, []), false, "无待办不应降频");
assert.equal(nextPollDelay(base, []), BASE_POLL_MS);

const pending = view([], seats);
(pending.player_area as unknown as Record<string, unknown>) = {
  seat: 0,
  player_id: "p0",
  pending: { kind: "wuxie", actor: "p0", options: [] },
};
assert.equal(awaitingLocal(pending, []), true, "本人 pending → 等待");
assert.equal(nextPollDelay(pending, []), AWAIT_POLL_MS);

const myTurn = view([], seats);
(myTurn.player_area as unknown as Record<string, unknown>) = {
  seat: 0,
  player_id: "p0",
  is_current: true,
};
assert.equal(awaitingLocal(myTurn, [{ id: "end_play", description: "结束出牌" }]), true);
assert.equal(awaitingLocal(myTurn, []), false);

const setup = view([], seats);
(setup.player_area as unknown as Record<string, unknown>) = {
  seat: 0,
  player_id: "p0",
  setup: { picked: null, candidate_details: [{ code: "a", name: "刘备" }] },
};
assert.equal(awaitingLocal(setup, []), true, "选将未选 → 等待");

// —— mock 时钟：待办态连续两个 60s tick，动作后立即回到 10s ——
let mockNow = 0;
const timeline: Array<{ at: number; delay: number }> = [];
let mocked: SgsViewData | null = pending;
for (let i = 0; i < 2; i += 1) {
  const delay = nextPollDelay(mocked, []);
  timeline.push({ at: mockNow, delay });
  mockNow += delay;
}
assert.deepEqual(
  timeline.map((x) => x.delay),
  [AWAIT_POLL_MS, AWAIT_POLL_MS],
  "待办态：连续两次 60s 保活",
);
mocked = myTurn;
const afterAction = nextPollDelay(mocked, []);
assert.equal(afterAction, BASE_POLL_MS, "离开待办态：下一次恢复 10s");
const beforeAction = mockNow;
mockNow += afterAction;
assert.equal(mockNow - beforeAction, BASE_POLL_MS, "动作后下一次 tick 只等 10s");

// —— 历史 → 特效 ——
assert.deepEqual(buildFxCues(null, view([hist(1, "x")])), [], "首帧不回放");

const prev = view([hist(1, "选将完成，对局开始")]);
const damage = view([
  hist(1, "选将完成，对局开始"),
  hist(2, "使用【杀♠7】", "p0"),
  hist(3, "p1 受到 2 点普通伤害（体力 2）"),
  hist(4, "使用【桃♥3】回复 1 点体力（3）", "p1"),
  hist(5, "打出【闪♥2】", "p2"),
  hist(6, "使用【酒♦9】：本回合下一张【杀】伤害 +1", "p0"),
]);
const cues = buildFxCues(prev, damage);
assert.deepEqual(kinds(cues), ["slash@1", "peach@1", "dodge@2", "wine@0"], "基础动作映射");

const thunder = view([
  hist(1, "选将完成，对局开始"),
  hist(2, "p1 受到 2 点雷电伤害（体力 1）"),
  hist(3, "p2 受到 1 点火焰伤害（体力 2）"),
]);
assert.deepEqual(kinds(buildFxCues(prev, thunder)), ["thunder@1", "fire@2"], "元素伤害映射");

const big = view([
  hist(1, "选将完成，对局开始"),
  hist(2, "雷击：判定【黑桃A】黑桃，造成 2 点雷电伤害"),
  hist(3, "p1 受到 2 点雷电伤害（体力 1）"),
]);
assert.equal(buildFxCues(prev, big)[0]?.variant, "big", "雷击加强变体");

const chain = view([
  hist(1, "选将完成，对局开始"),
  hist(2, "铁索连环：对 曹操、孙权 结算"),
  hist(3, "铁索连环：属性伤害传导至 2 名横置角色"),
  hist(4, "p1 受到 2 点火焰伤害（体力 1）"),
  hist(5, "p2 受到 2 点火焰伤害（体力 1）"),
]);
assert.deepEqual(
  kinds(buildFxCues(prev, chain)),
  ["chain@1", "chain@2", "chain@1", "fire@1", "chain@2", "fire@2"],
  "铁索点位与传导逐个链缚",
);

// 合并：同目标同类型连续伤害
const merged = view([
  hist(1, "选将完成，对局开始"),
  hist(2, "p1 受到 1 点普通伤害（体力 3）"),
  hist(3, "p1 受到 1 点普通伤害（体力 2）"),
]);
const m = buildFxCues(prev, merged);
assert.deepEqual(kinds(m), ["slash@1×2"], "连续同类伤害合并");
assert.equal(m[0]?.amount, 2);

// 超过 12 条只保留最近 12 条
const many: SgsHistoryEntry[] = [hist(1, "选将完成，对局开始")];
for (let i = 0; i < 20; i += 1) many.push(hist(i + 2, `p${i % 2} 受到 1 点普通伤害（体力 3）`));
const capped = buildFxCues(prev, view(many));
assert.equal(capped.length, 12, "上限 12 条");
assert.equal(capped[capped.length - 1]?.seq, 21, "保留最近条目");

// HP 差分兜底：无伤害日志也应有受击
const hpDown = seats.map((s) => (s.seat === 1 ? { ...s, hp: 2 } : s));
const fallback = buildFxCues(prev, view([hist(1, "选将完成，对局开始"), hist(2, "任意日志")], hpDown));
assert.deepEqual(kinds(fallback), ["slash@1"], "HP 差分兜底");

assert.equal(historyLastSeq(damage), 6, "末条 seq");

// —— 签名：仅 history 变化时不重建 app；侧栏单独更新 ——
const baseView = view([hist(1, "a"), hist(2, "b")]);
const histOnly = view([hist(1, "a"), hist(2, "b"), hist(3, "c")]);
assert.equal(
  stateSignature(baseView, []),
  stateSignature(histOnly, []),
  "仅有 history 变化时 appSig 不变",
);
assert.notEqual(sideSignature(baseView), sideSignature(histOnly), "sideSig 随 history 变化");
const hpChanged = view([hist(1, "a"), hist(2, "b")], seats.map((s) => (s.seat === 1 ? { ...s, hp: 3 } : s)));
assert.notEqual(stateSignature(baseView, []), stateSignature(hpChanged, []), "HP 变化触发 appSig");

console.log("[sgs-fx] 特效/轮询纯函数单测通过 ✅");
