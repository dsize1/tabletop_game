// 三国杀 Q 群侧命令白名单与文案单测（P7 最小化，ADR-0031）。
// 运行：npx tsx test-sanguosha-chat.mts
import assert from "node:assert";
import {
  SGS_NO_GAME,
  isSgsChatCommandAllowed,
  sgsBoardText,
  sgsBotsAdded,
  sgsGeneralPackApplied,
  sgsGeneralPackMenu,
  sgsLinkList,
  sgsLobbyPrompt,
  sgsRefuseHint,
  sgsSeatCount,
  sgsStartedPrompt,
} from "./src/sanguosha-chat.js";

// —— 白名单：只允许大厅/查询类命令（P6.3 起放开机器人补位与武将包） ——
for (const cmd of [
  "create",
  "join",
  "leave",
  "start",
  "abort",
  "board",
  "resume",
  "kick",
  "addbot",
  "generalpacks",
] as const) {
  assert.ok(isSgsChatCommandAllowed(cmd), `${cmd} 应在白名单`);
}
for (const cmd of [
  "pick",
  "pickcode",
  "sg",
  "sgrule",
  "setsg",
  "place",
  "reroll",
  "smallgames",
  "solo",
] as const) {
  assert.ok(!isSgsChatCommandAllowed(cmd), `${cmd} 必须被拒绝（群内不做逐动作交互）`);
}

// —— 文案：不得出现拉斯维加斯专属内容 ——
const lobby = sgsLobbyPrompt("甲", 3);
assert.ok(lobby.includes("#加机器人"), "大厅引导应提示机器人补位（P6.3）");
assert.ok(lobby.includes("#武将包"), "大厅引导应提示武将包设置（P6.3）");
assert.ok(lobby.includes("三国杀"), lobby);
assert.ok(lobby.includes("甲"), lobby);
assert.ok(lobby.includes("3 人"), lobby);
assert.ok(lobby.includes("#加入") && lobby.includes("#开始") && lobby.includes("#中断"), lobby);
assert.ok(!lobby.includes("小游戏"), "三国杀引导不得出现 #小游戏");
assert.ok(!lobby.includes("筹码"), "三国杀引导不得出现筹码");

const started = sgsStartedPrompt(5);
assert.ok(started.includes("5 人"), started);
assert.ok(started.includes("网页"), started);

// —— 局面：一行播报 + 可选链接 ——
const board = sgsBoardText("三国杀（进行中）：第 3 轮，轮到 甲，牌堆余 12", "https://x/login?t=1");
assert.ok(board.split("\n")[0].includes("第 3 轮"), board);
assert.ok(board.includes("https://x/login?t=1"), board);
assert.equal(sgsBoardText(""), "三国杀：进行中");

// —— 拒绝提示：引导到网页，不回显合法动作 ——
const refuse = sgsRefuseHint("https://x/login?t=2");
assert.ok(refuse.includes("网页"), refuse);
assert.ok(refuse.includes("https://x/login?t=2"), refuse);
assert.ok(refuse.includes("#局面"), refuse);
assert.ok(!/legal|可选动作|1\)/.test(refuse), "拒绝提示不得列动作清单");

// —— 链接列表 ——
const list = sgsLinkList("标题", [
  { name: "甲", url: "https://x/1" },
  { name: "乙", url: "https://x/2" },
]);
assert.ok(list.startsWith("标题"), list);
assert.equal(list.split("\n").length, 3);
assert.ok(list.includes("甲：https://x/1"), list);
assert.equal(sgsLinkList("只有标题", []), "只有标题");

assert.ok(SGS_NO_GAME.includes("三国杀"), SGS_NO_GAME);

console.log("✅ test-sanguosha-chat 通过");

// —— P6.3：机器人补位 / 武将包文案 ——
const bots = sgsBotsAdded(3, 4);
assert.ok(bots.includes("3 个机器人"), bots);
assert.ok(bots.includes("4/8"), bots);
assert.ok(bots.includes("#开始"), bots);

const menu = sgsGeneralPackMenu([
  { label: "武将包：全部（可用 149 位）" },
  { label: "武将包：标准（可用 25 位）" },
]);
assert.ok(menu.includes("1) 武将包：全部"), menu);
assert.ok(menu.includes("2) 武将包：标准"), menu);
assert.ok(menu.includes("all"), menu);
assert.equal(sgsGeneralPackMenu([]).includes("没有可切换"), true);

const applied = sgsGeneralPackApplied("武将包：全部（可用 149 位）");
assert.ok(applied.startsWith("✅"), applied);
assert.ok(applied.includes("#开始"), applied);

// —— P6.3：座位数要兼容「原始 state」与「视图投影」两种形状 ——
assert.equal(
  sgsSeatCount({ seats: [{ player_id: "a" }, { player_id: "b" }] }),
  2,
  "原始 state 的 data.seats",
);
assert.equal(
  sgsSeatCount({ public_board: { seats: [{}, {}, {}, {}, {}] } }),
  5,
  "公共视图的 data.public_board.seats（#加机器人 0/8 的回归）",
);
assert.equal(sgsSeatCount(undefined), 0);
assert.equal(sgsSeatCount({}), 0);
