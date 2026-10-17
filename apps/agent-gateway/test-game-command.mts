// 拉斯维加斯游戏命令解析层单测：显式前缀 → 命令 → MCP 动作串。
// 运行：npx tsx test-game-command.mts
import assert from "node:assert";
import {
  parseGameCommand,
  gameActionId,
  lobbyPrompt,
  formatActionList,
  GAME_ACTION_HINT,
  GAME_ACTION_CHOICE_LIMIT,
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
// 机器人补位";
assert.deepEqual(parseGameCommand("#加机器人"), { cmd: "addbot", n: 1 });
assert.deepEqual(parseGameCommand("#加机器人 2"), { cmd: "addbot", n: 2 });
assert.deepEqual(parseGameCommand("#机器人 3"), { cmd: "addbot", n: 3 });
assert.deepEqual(parseGameCommand("#单人"), { cmd: "solo", n: 3 });
assert.deepEqual(parseGameCommand("#单人 2"), { cmd: "solo", n: 2 });
// 通用小游戏动作兜底
assert.deepEqual(parseGameCommand("#动作 F1:move:0:3"), { cmd: "sg", id: "F1:move:0:3" });
// 小游戏规则查询（大小写不敏感）
assert.deepEqual(parseGameCommand("#小游戏#f1"), { cmd: "sgrule", id: "f1" });
assert.deepEqual(parseGameCommand("#小游戏#F1"), { cmd: "sgrule", id: "F1" });
assert.deepEqual(parseGameCommand("#规则 f2"), { cmd: "sgrule", id: "f2" });
assert.deepEqual(parseGameCommand("#小游戏 4"), { cmd: "smallgames", n: 4 });
assert.deepEqual(parseGameCommand("#sg sg:A1:guess:5"), { cmd: "sg", id: "sg:A1:guess:5" });
assert.equal(parseGameCommand("#单人 9"), null);

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
assert.equal(gameActionId({ cmd: "addbot", n: 2 }), "lobby:add_bot");
assert.equal(gameActionId({ cmd: "sg", id: "F1:move:0:3" }), "sg:F1:move:0:3");
assert.equal(gameActionId({ cmd: "solo", n: 3 }), null);
assert.equal(gameActionId({ cmd: "create" }), null);
assert.equal(gameActionId({ cmd: "board" }), null);

// —— 文案 ——
const prompt = lobbyPrompt("甲", 2, 3);
assert.ok(prompt.includes("甲"), "引导应含发起人");
assert.ok(prompt.includes("#开始"), "引导应含开赛命令");
assert.ok(prompt.includes("#加入"), "引导应含加入命令");
assert.ok(GAME_ACTION_HINT.includes("#放"), "在局引导应含下注命令");

// —— 合法动作清单渲染（阶段 4b 兑底） ——
assert.equal(formatActionList([]), "当前没有可执行的动作。");
const list = formatActionList([
  { id: "place:3", description: "把 3 点全部放到 3 号赌场" },
  { id: "reroll", description: "支付 1 筹码弃回合" },
]);
assert.ok(list.includes("1) 把 3 点全部放到 3 号赌场"), list);
assert.ok(list.includes("2) 支付 1 筹码弃回合"), list);
assert.ok(list.includes("#放 3"), list);
assert.equal(GAME_ACTION_CHOICE_LIMIT, 26);

console.log("✅ test-game-command 通过");
