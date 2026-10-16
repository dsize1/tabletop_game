// 决策层单测：SystemOne 客户端（注入桩，不依赖网络）+ 前缀解析 + 兜底 + 手势提取。
// 运行：npx tsx test-decision.mts
import assert from "node:assert";
import {
  decideIntent,
  parsePrefix,
  extractHand,
  maskUrls,
  confidenceBand,
  normalizeThresholds,
  INTENT_CRITERIA,
} from "./src/decision.js";
import { SystemOneClient } from "./src/systemone.js";
import { stubClient, failingClient, hangingClient } from "./test-helpers.mts";

// —— 前缀解析 ——
assert.equal(parsePrefix("#聊 你好"), "chat");
assert.equal(parsePrefix("#玩 猜拳"), "start_game");
assert.equal(parsePrefix("#开 猜拳"), "start_game");
assert.equal(parsePrefix("#play 猜拳"), "start_game");
assert.equal(parsePrefix("#start 猜拳"), "start_game");
assert.equal(parsePrefix("#开发 测试"), "dev"); // 回归：#开发 不应被 #开 抢先命中
assert.equal(parsePrefix("随便说说"), null);

// —— 手势提取 ——
assert.equal(extractHand("来玩猜拳 石头"), "石头");
assert.equal(extractHand("我出剪刀"), "剪刀");
assert.equal(extractHand("布"), "布");
assert.equal(extractHand("随便说点啥"), null);

// —— 高置信 → via=decision ——
{
  const d = await decideIntent("来玩猜拳", stubClient("start_game", 0.92));
  assert.equal(d.intent, "start_game");
  assert.equal(d.via, "decision");
  assert.equal(d.confidence, 0.92);
}

// —— 低置信 + 前缀 → via=prefix ——
{
  const d = await decideIntent("#聊 随便聊聊", stubClient("start_game", 0.3));
  assert.equal(d.intent, "chat");
  assert.equal(d.via, "prefix");
}

// —— 低置信 + 无前缀 → 默认 chat，via=default ——
{
  const d = await decideIntent("随便说点啥", stubClient("chat", 0.41));
  assert.equal(d.intent, "chat");
  assert.equal(d.via, "default");
}

// —— 阈值可配：同一置信度阈值调低则走 decision ——
{
  const d = await decideIntent("嗯", stubClient("dev", 0.5), { threshold: 0.4, thresholdHigh: 0.4 });
  assert.equal(d.intent, "dev");
  assert.equal(d.via, "decision");
}

// —— 置信度分档：[low, high) 为 confirm（左闭右开）——
assert.equal(confidenceBand(0.9, 0.6, 0.8), "high");
assert.equal(confidenceBand(0.8, 0.6, 0.8), "high");
assert.equal(confidenceBand(0.6, 0.6, 0.8), "confirm");
assert.equal(confidenceBand(0.799, 0.6, 0.8), "confirm");
assert.equal(confidenceBand(0.599, 0.6, 0.8), "low");

// —— 阈值规整：倒挂夹紧、非数值回退默认 ——
assert.deepEqual(normalizeThresholds(0.6, 0.8), { low: 0.6, high: 0.8 });
assert.deepEqual(normalizeThresholds(0.7, 0.5), { low: 0.7, high: 0.7 });
assert.deepEqual(normalizeThresholds(Number.NaN, Number.NaN), { low: 0.6, high: 0.8 });

// —— 中带 + 非 chat 候选 → via=confirm + model ——
{
  const d = await decideIntent("帮评估下复杂度", stubClient("dev", 0.72));
  assert.equal(d.via, "confirm");
  assert.equal(d.intent, "dev");
  assert.deepEqual(d.model, { intent: "dev", confidence: 0.72 });
}
// —— 中带 + 候选=chat → 免确认，走默认兜底（仍保留 model 供日志回溯）——
{
  const d = await decideIntent("随便说说", stubClient("chat", 0.7));
  assert.equal(d.via, "default");
  assert.equal(d.intent, "chat");
  assert.deepEqual(d.model, { intent: "chat", confidence: 0.7 });
}
// —— 低阈值分支仍走前缀/默认（回归）——
{
  const d = await decideIntent("#玩 猜拳", stubClient("dev", 0.5));
  assert.equal(d.via, "prefix");
  assert.equal(d.intent, "start_game");
}

// —— 服务连不上 → via=unavailable（不降级为 chat）——
{
  const d = await decideIntent("来玩猜拳", failingClient());
  assert.equal(d.via, "unavailable");
}

// —— 服务超时 → via=unavailable ——
{
  const d = await decideIntent("来玩猜拳", hangingClient(200));
  assert.equal(d.via, "unavailable");
}

// —— 模型返回未知意图 → 走兜底 ——
{
  const d = await decideIntent("???", stubClient("nonsense", 0.99));
  assert.equal(d.via, "default");
  assert.equal(d.intent, "chat");
}

// —— 意图选项齐全 ——
assert.deepEqual(Object.keys(INTENT_CRITERIA).sort(), ["chat", "dev", "game_action", "start_game"]);

// —— 意图分类前屏蔽 URL（避免 /dev_docs 等链接文本污染意图）——
assert.equal(maskUrls("介绍下 https://x.com/dev_docs 该链接"), "介绍下 [链接] 该链接");
assert.equal(maskUrls("看这个 http://a.cn/x。"), "看这个 [链接]。");
assert.equal(maskUrls("无链接"), "无链接");
{
  let sent = "";
  const fetchImpl = (async (_url: string, init?: RequestInit) => {
    sent = JSON.parse(String(init?.body)).state.message;
    return new Response(
      JSON.stringify({ answers: { intent: { type: "choice", choice: "chat", confidence: 0.9 } } }),
      { status: 200 },
    );
  }) as unknown as typeof fetch;
  const c = new SystemOneClient({ url: "http://stub", timeoutMs: 1000, fetchImpl });
  const d = await decideIntent("介绍下 https://x.com/dev_docs 内容", c);
  assert.equal(d.intent, "chat");
  assert.ok(!sent.includes("dev_docs"), `送模型的消息不应含 URL：${sent}`);
  assert.ok(sent.includes("[链接]"), `送模型的消息应含占位符：${sent}`);
}

console.log("✅ decision 单测全部通过");
