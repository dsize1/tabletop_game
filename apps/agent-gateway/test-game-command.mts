// 拉斯维加斯游戏命令解析层单测：显式前缀 → 命令 → MCP 动作串。
// 运行：npx tsx test-game-command.mts
import assert from "node:assert";
import {
  parseGameCommand,
  gameActionId,
  lobbyPrompt,
  GAME_ACTION_HINT,
} from "./src/game-command.js";

// —— 命令解析（显式前缀） ——
assert.deepEqual(parseGameCommand("#开局"), { cmd: "create" });
assert.deepEqual(parseGameCommand("#拉斯维加斯"), { cmd: "create" });
assert.deepEqual(parseGameCommand("#加入"), { cmd: "join" });
assert.deepEqual(parseGameCommand("#退出"), { cmd: "leave" });
assert.deepEqual(parseGameCommand("#开始"), { cmd: "start" });
assert.deepEqual(parseGameCommand("#中断"), { cmd: "abort" });
assert.deepEqual(parseGameCommand("#踢 abc123"), { cmd: "kick", target: "abc123" });
assert.deepEqual(parseGameCommand("#小游戏 4"), { cmd: "smallgames", n: 4 });
assert.deepEqual(parseGameCommand("#小游戏4"), { cmd: "smallgames", n: 4 });
assert.deepEqual(parseGameCommand("#放 3"), { cmd: "place", point: 3 });
assert.deepEqual(parseGameCommand("#放3"), { cmd: "place", point: 3 });
assert.deepEqual(parseGameCommand("#重投"), { cmd: "reroll" });
assert.deepEqual(parseGameCommand("#弃"), { cmd: "reroll" });
assert.deepEqual(parseGameCommand("#局面"), { cmd: "board" });
assert.deepEqual(parseGameCommand("  #加入  "), { cmd: "join" });

// —— 非法参数 → null（回落决策模型，不误发动作） ——
assert.equal(parseGameCommand("#放 0"), null);
assert.equal(parseGameCommand("#放 7"), null);
assert.equal(parseGameCommand("#小游戏 2"), null);
assert.equal(parseGameCommand("#小游戏 7"), null);
assert.equal(parseGameCommand("#踢"), null);

// —— 非游戏命令不命中 ——
assert.equal(parseGameCommand("今天天气不错"), null);
assert.equal(parseGameCommand("放3"), null); // 必须带 #
assert.equal(parseGameCommand("#开发 做个功能"), null);
assert.equal(parseGameCommand("#任务"), null);
assert.equal(parseGameCommand("#进度 abc"), null);
assert.equal(parseGameCommand(""), null);

// —— 动作映射（与引擎 legal-actions 一致） ——
assert.equal(gameActionId({ cmd: "join" }), "lobby:join");
assert.equal(gameActionId({ cmd: "leave" }), "lobby:leave");
assert.equal(gameActionId({ cmd: "start" }), "lobby:start");
assert.equal(gameActionId({ cmd: "abort" }), "lobby:abort");
assert.equal(gameActionId({ cmd: "kick", target: "u1" }), "lobby:kick:u1");
assert.equal(gameActionId({ cmd: "smallgames", n: 5 }), "lobby:set_small_game_count:5");
assert.equal(gameActionId({ cmd: "place", point: 6 }), "place:6");
assert.equal(gameActionId({ cmd: "reroll" }), "reroll");
assert.equal(gameActionId({ cmd: "create" }), null);
assert.equal(gameActionId({ cmd: "board" }), null);

// —— 文案 ——
const prompt = lobbyPrompt("甲", 2, 3);
assert.ok(prompt.includes("甲"), "引导应含发起人");
assert.ok(prompt.includes("#开始"), "引导应含开赛命令");
assert.ok(prompt.includes("#加入"), "引导应含加入命令");
assert.ok(GAME_ACTION_HINT.includes("#放"), "在局引导应含下注命令");

console.log("✅ test-game-command 通过");
