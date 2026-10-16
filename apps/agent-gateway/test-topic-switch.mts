// 话题切换单测：门控（gap）→ 同一次前向多问 topic_switch → A 边界标记 / C 历史元数据。
// 运行：npx tsx test-topic-switch.mts
//
// 范围说明：index.ts 的编排只做「读门控 → 传 boundary → 写 history」的接线，其判据与渲染
// 分别在此（isStaleGap / isTopicSwitch / chat-history / renderHistory）验证。
import assert from "node:assert";
import {
  DEFAULT_TOPIC_SWITCH_THRESHOLD,
  decideIntent,
  isTopicSwitch,
} from "./src/decision.js";
import {
  TOPIC_BOUNDARY_MARK,
  chatGapMs,
  isStaleGap,
  renderHistory,
  type TimedTurn,
} from "./src/time-context.js";
import {
  appendChatHistory,
  boundaryDisposition,
  chatHistoryKey,
  clearPendingBoundary,
  consumePendingBoundary,
  getChatHistory,
  markPendingBoundary,
  pendingBoundaryKey,
} from "./src/chat-history.js";
import {
  assertNoInternalMarkers,
  memKVStore,
  memListStore,
  recordingClient,
} from "./test-helpers.mts";

const GATE = 30 * 60_000; // 30min（默认门控）
const NOW = Date.parse("2026-02-14T01:12:00Z");

// ==================== 1) 门控：just gap，无模型参与 ====================
assert.equal(isStaleGap(undefined, GATE), false, "无历史 → 不做话题判定");
assert.equal(isStaleGap(29 * 60_000, GATE), false, "未达门控 → 不做话题判定");
assert.equal(isStaleGap(30 * 60_000, GATE), true, "达门控（左闭）→ 做话题判定");
assert.equal(isStaleGap(2 * 3600_000, GATE), true, "跨夜 → 做话题判定");
assert.equal(isStaleGap(2 * 3600_000, 0), false, "门控关闭（TOPIC_GAP_MINUTES=0）");

// ==================== 2) 同一次前向多问（零额外 RTT 的不变量） ====================
{
  const { client, calls } = recordingClient({
    intent: { type: "choice", choice: "chat", confidence: 0.95 },
    topic_switch: { type: "noul", noul: 0.88 },
  });
  const d = await decideIntent("早", client, {
    askTopicSwitch: true,
    previousMessage: "今天先到这",
  });
  assert.equal(d.intent, "chat");
  assert.equal(d.via, "decision");
  assert.equal(d.topicSwitch, 0.88);
  // 关键回归：两次问题仍在**一次** HTTP 请求 / 一次前向内
  assert.equal(calls.length, 1, "topic_switch 必须并入既有单次请求");
  const q = calls[0]!.questions as Record<string, { type: string }>;
  assert.equal(q.intent?.type, "choice");
  assert.equal(q.topic_switch?.type, "noul");
  assert.equal(calls[0]!.state.previous_message, "今天先到这");
}
// 不问时不追加问题，且不返回 topicSwitch
{
  const { client, calls } = recordingClient({
    intent: { type: "choice", choice: "chat", confidence: 0.95 },
  });
  const d = await decideIntent("随便说说", client);
  assert.equal(d.topicSwitch, undefined);
  assert.equal(calls.length, 1);
  assert.equal("topic_switch" in (calls[0]!.questions as object), false);
}
// 决策服务连不上 → 无 topicSwitch（调用方按「不切换」处理，安全降级）
{
  const { client } = recordingClient({});
  const bad = new (await import("./src/systemone.js")).SystemOneClient({
    url: "http://stub",
    timeoutMs: 1000,
    fetchImpl: (async () => {
      throw new Error("ECONNREFUSED");
    }) as unknown as typeof fetch,
  });
  const d = await decideIntent("早", bad, { askTopicSwitch: true });
  assert.equal(d.via, "unavailable");
  assert.equal(d.topicSwitch, undefined);
  void client;
}

// ==================== 3) A：阈值判定 + 边界标记渲染 ====================
assert.equal(DEFAULT_TOPIC_SWITCH_THRESHOLD, 0.7);
assert.equal(isTopicSwitch(undefined, 0.7), false);
assert.equal(isTopicSwitch(0.69, 0.7), false);
assert.equal(isTopicSwitch(0.7, 0.7), true); // 左闭
assert.equal(isTopicSwitch(0.95, 0.7), true);
assert.equal(isTopicSwitch(Number.NaN, 0.7), false);
{
  const boundary = isTopicSwitch(0.88);
  const turns: TimedTurn[] = [
    { role: "user", content: "今天先到这", ts: NOW - 15 * 3600_000 },
    { role: "assistant", content: "好，晚安", ts: NOW - 15 * 3600_000 + 60_000 },
    { role: "user", content: "早", ts: NOW, ...(boundary ? { boundary: true } : {}) },
  ];
  const out = renderHistory(turns);
  assert.equal(out[2]!.content, `${TOPIC_BOUNDARY_MARK}\n[02-14 09:12] 早`);
  // 边界标记只在检出切换的那一轮，历史原样
  assert.equal(out[0]!.content, "[02-13 18:12] 今天先到这");
  assert.equal(out[1]!.content, "好，晚安");
}
// 未切换 → 无边界标记（仍可能有时间锚点）
{
  const turns: TimedTurn[] = [
    { role: "user", content: "今天先到这", ts: NOW - 15 * 3600_000 },
    { role: "user", content: "早", ts: NOW },
  ];
  const out = renderHistory(turns);
  assert.ok(!out[1]!.content.includes(TOPIC_BOUNDARY_MARK));
}

// ==================== 4) 历史读写：元数据共存 + 旧格式降级 + 无时间文本出站 ====================
{
  const store = memListStore();
  const G = "g1";

  // 旧格式条目（无 ts/boundary）→ 降级为「时间未知」，不抛错
  await store.rpush(chatHistoryKey(G), JSON.stringify({ role: "user", content: "旧消息" }));
  // 新格式：带 ts + boundary
  await appendChatHistory(store, G, "user", "早", { ts: NOW, boundary: true }, 20);
  await appendChatHistory(store, G, "assistant", "早上好", { ts: NOW + 1000 }, 20);

  const history = await getChatHistory(store, G, 20);
  assert.equal(history.length, 3);
  assert.equal(history[0]!.ts, undefined, "旧条目 ts 应缺失");
  assert.equal(history[0]!.boundary, undefined);
  assert.equal(history[1]!.ts, NOW);
  assert.equal(history[1]!.boundary, true);
  assert.equal(history[2]!.ts, NOW + 1000);
  assert.equal(history[2]!.boundary, undefined);

  // 存储的 content 必须是原始消息，不含时间锚点 / 内部标记 / 时间块（时间只入上下文）
  const raw = store.map.get(chatHistoryKey(G))!;
  for (const r of raw) assertNoInternalMarkers(r, "chat:history 条目");
  assert.deepEqual(
    raw.map((r) => JSON.parse(r).content),
    ["旧消息", "早", "早上好"],
  );

  // 门控据此计算（最后一条 ts = NOW+1000）
  assert.equal(chatGapMs(history, NOW + 1000 + 45 * 60_000), 45 * 60_000);
}

// ==================== 5) 条数上限裁剪 ====================
{
  const store = memListStore();
  for (let i = 0; i < 5; i++) {
    await appendChatHistory(store, "g2", "user", `m${i}`, { ts: NOW + i }, 3);
  }
  const history = await getChatHistory(store, "g2", 3);
  assert.deepEqual(history.map((t) => t.content), ["m2", "m3", "m4"]);
}

// ==================== 6) C：门控命中 → pending 失效（判据与 index 接线一致）====================
{
  // handle() 的判定：staleGap === isStaleGap(gapMs, TOPIC_GAP_MS)
  // pending 存在且 staleGap → clearPending；否则走确认流程。
  const staleWithPending = isStaleGap(31 * 60_000, GATE);
  assert.equal(staleWithPending, true, "pending 应被丢弃而不是把新消息当 y/n 消费");
  const freshWithPending = isStaleGap(2 * 60_000, GATE);
  assert.equal(freshWithPending, false, "窗口内仍按确认流程处理");
}

// ==================== 7) 边界去向：chat 直接落 / 其它暂存 ====================
assert.equal(boundaryDisposition(false, "chat"), "none");
assert.equal(boundaryDisposition(false, "start_game"), "none");
assert.equal(boundaryDisposition(true, "chat"), "history");
assert.equal(boundaryDisposition(true, "start_game"), "stash");
assert.equal(boundaryDisposition(true, "game_action"), "stash");
assert.equal(boundaryDisposition(true, "dev"), "stash");

// ==================== 8) 待落位边界：暂存 → 下一条 chat 消息取用一次 ====================
{
  const kv = memKVStore();
  const G = "g-boundary";

  // 未暂存时取用为空
  assert.equal(await consumePendingBoundary(kv, G), false);

  // 暂存后取用一次为真，且被清除（不重复落位）
  await markPendingBoundary(kv, G);
  assert.equal(kv.map.has(pendingBoundaryKey(G)), true);
  assert.equal(await consumePendingBoundary(kv, G), true);
  assert.equal(await consumePendingBoundary(kv, G), false);
  assert.equal(kv.map.has(pendingBoundaryKey(G)), false);

  // 门控重新判定时以本次为准 → 丢弃旧暂存
  await markPendingBoundary(kv, G);
  await clearPendingBoundary(kv, G);
  assert.equal(await consumePendingBoundary(kv, G), false);

  // 端到端（list + kv）：非 chat 轮暂存 → 下一条 chat 轮落位到 history
  const store = memListStore();
  await appendChatHistory(store, G, "user", "今天先到这", { ts: NOW - 15 * 3600_000 });
  if (boundaryDisposition(true, "start_game") === "stash") await markPendingBoundary(kv, G);
  const carried = await consumePendingBoundary(kv, G);
  assert.equal(carried, true);
  await appendChatHistory(store, G, "user", "早", { ts: NOW, boundary: carried }, 20);
  const rendered = renderHistory(await getChatHistory(store, G, 20));
  assert.equal(rendered[1]!.content, `${TOPIC_BOUNDARY_MARK}\n[02-14 09:12] 早`);
}

console.log("✅ topic-switch 单测全部通过");
