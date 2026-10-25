// 桌游目录单测：`#开局` 菜单渲染 + 编号/名称/别名解析。
// 运行：npx tsx test-games.mts
import assert from "node:assert";
import {
  GAME_OPTIONS,
  directLaunchPrompt,
  formatGameMenu,
  resolveGameOption,
} from "./src/games.js";

// —— 目录：至少含已有引擎，id 唯一 ——
assert.ok(GAME_OPTIONS.length >= 2, "至少有拉斯维加斯与猜拳两个选项");
assert.equal(new Set(GAME_OPTIONS.map((g) => g.id)).size, GAME_OPTIONS.length, "id 必须唯一");
assert.ok(GAME_OPTIONS.some((g) => g.id === "las-vegas"), "应含拉斯维加斯");
assert.ok(GAME_OPTIONS.some((g) => g.id === "rps"), "应含猜拳");

// —— 菜单：逐项编号，可作 `#开局 <编号>` ——
const menu = formatGameMenu();
assert.ok(menu.includes("#开局"), menu);
for (const [i, g] of GAME_OPTIONS.entries()) {
  assert.ok(menu.includes(`${i + 1}) ${g.name}`), `菜单应含编号项 ${i + 1}: ${g.name}`);
}

// —— 解析：编号（1 起） ——
assert.equal(resolveGameOption("1")?.id, GAME_OPTIONS[0].id);
assert.equal(resolveGameOption("2")?.id, GAME_OPTIONS[1].id);
assert.equal(resolveGameOption("0"), null);
assert.equal(resolveGameOption(String(GAME_OPTIONS.length + 1)), null);

// —— 解析：id / 展示名 / 别名，大小写不敏感 ——
assert.equal(resolveGameOption("las-vegas")?.id, "las-vegas");
assert.equal(resolveGameOption("拉斯维加斯")?.id, "las-vegas");
assert.equal(resolveGameOption("RPS")?.id, "rps");
assert.equal(resolveGameOption("猜拳")?.id, "rps");
assert.equal(resolveGameOption("石头剪刀布")?.id, "rps");
assert.equal(resolveGameOption("  #2  ")?.id, GAME_OPTIONS[1].id);

// —— 未命中：空 / 未知名称 ——
assert.equal(resolveGameOption(), null);
assert.equal(resolveGameOption(""), null);
assert.equal(resolveGameOption("围棋"), null);

// —— direct 模式引导 ——
const rps = GAME_OPTIONS.find((g) => g.id === "rps")!;
const prompt = directLaunchPrompt(rps, "甲");
assert.ok(prompt.includes("甲"), prompt);
assert.ok(prompt.includes("石头"), prompt);
assert.ok(prompt.includes("剪刀"), prompt);

console.log("✅ test-games 通过");
