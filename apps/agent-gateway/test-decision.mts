// 决策层单测：前缀解析 + mock 决策 + 交叉验证 + 手势提取。
// 运行：npx tsx test-decision.mts
import assert from "node:assert";
import { decideIntent, extractHand } from "./src/decision.js";

// —— 前缀解析 ——
assert.equal(decideIntent("#聊 你好").intent, "chat");
assert.equal(decideIntent("#玩 猜拳").intent, "start_game");
assert.equal(decideIntent("#开 猜拳").intent, "start_game");
assert.equal(decideIntent("#play 猜拳").intent, "start_game");
assert.equal(decideIntent("#start 猜拳").intent, "start_game");
assert.equal(decideIntent("#开发 测试").intent, "dev");

// —— mock 决策（关键词）——
assert.equal(decideIntent("来玩猜拳").intent, "start_game");
assert.equal(decideIntent("石头剪刀布").intent, "start_game");
assert.equal(decideIntent("石头").intent, "game_action");
assert.equal(decideIntent("开发个功能").intent, "dev");

// —— 交叉验证：决策高置信 → via=decision ——
assert.equal(decideIntent("来玩猜拳").via, "decision");

// —— 低置信 + 前缀兜底 → via=prefix ——
assert.equal(decideIntent("#聊 随便聊聊").via, "prefix");

// —— 低置信 + 无前缀 → default chat ——
assert.equal(decideIntent("随便说点啥").intent, "chat");
assert.equal(decideIntent("随便说点啥").via, "default");

// —— 手势提取 ——
assert.equal(extractHand("来玩猜拳 石头"), "石头");
assert.equal(extractHand("我出剪刀"), "剪刀");
assert.equal(extractHand("布"), "布");
assert.equal(extractHand("随便说点啥"), null);

console.log("✅ decision 单测全部通过");
