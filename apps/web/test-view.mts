// 版图/双模式渲染单测（纯函数，无需起服务）。
// 运行：tsx test-view.mts
import assert from "node:assert";
import { buildBoardScene, type SceneNode } from "./src/view/board/scene.js";
import { renderBoardSvg } from "./src/view/board/svg.js";
import { renderGameTextBody } from "./src/view/text.js";
import { renderAppPage } from "./src/view/page.js";
import type { GameView } from "./src/view/types.js";

const view: GameView = {
  game_id: "las-vegas",
  room_id: "r1",
  openid: "u1",
  public: {
    status: "active",
    players: ["u1", "u2", "u3", "u4"],
    data: {
      phase: "turn_roll",
      round: 2,
      turn: 1,
      seats: [
        { player_id: "u1", small_remaining: 0, big_remaining: 1, chips: 2, banknotes: [30_000] },
        { player_id: "u2", small_remaining: 3, big_remaining: 0, chips: 4, banknotes: [] },
        { player_id: "u3", small_remaining: 1, big_remaining: 1, chips: 1, banknotes: [], is_bot: true },
        { player_id: "u4", small_remaining: 2, big_remaining: 1, chips: 0, banknotes: [50_000, 30_000] },
      ],
      casinos: [
        { dice: { u1: 3, gray: 2 }, big_dice: { u1: 1 }, banknotes: [0, 0], small_games: ["F1"] },
        { dice: { u2: 1 }, banknotes: [0, 0], small_games: ["F2"] },
        { dice: { u3: 2, u4: 1, u1: 1 }, banknotes: [0, 0], small_games: [] },
        { dice: {}, banknotes: [], small_games: [] },
        { dice: { u4: 1 }, banknotes: [0], small_games: [] },
        { dice: { u2: 2 }, banknotes: [0, 0], small_games: [] },
      ],
      roll: { small: [1, 4, 5], big: 6 },
      sg: { gray_groups: [3, 2], lottery: [0, 0], banned: null, arrow: 1, bars: { u2: { small: 1 } } },
    },
  },
  player: {
    data: {
      sg: {
        pending_options: {
          game: "A1",
          stage: "hide",
          multi: false,
          options: [{ code: "K7Q2", count: 1, reward: "2 筹码" }],
        },
      },
    },
  },
  names: { u1: "轮子总比困难多", u2: "<b>机器人1</b>", u3: "bot3", u4: "四号" },
};

// —— 场景构建 ——
const scene = buildBoardScene(view);
assert.equal(scene.w, 760, "画布宽固定 760（手机竖屏优先）");
assert.ok(scene.h > 1000, "画布高度随玩家数增长");
assert.deepEqual(
  scene.layers.map((l) => l.name),
  ["bg", "tray", "casinos", "players", "games", "pools", "overlay"],
  "分层顺序固定",
);
assert.equal(scene.meta.seatCount, 4, "meta 记录座位数");
assert.equal(scene.meta.turnPlayerId, "u2", "meta 记录当前玩家");

// 6 个赌场恒在（空态也要画出来）
const casinoLayer = scene.layers.find((l) => l.name === "casinos")!;
const casinoTexts = casinoLayer.nodes.filter((n) => n.t === "text").map((n) => (n as { text: string }).text);
for (let i = 1; i <= 6; i += 1) {
  assert.ok(casinoTexts.includes(String(i)), `应画出 ${i} 号赌场编号`);
}
assert.ok(casinoTexts.some((t) => t.endsWith("张")), "钞票只显示张数");
assert.ok(!JSON.stringify(scene).includes("30000"), "版图不得出现钞票面额");

// 灰度骰、大骰、小游戏槽
assert.ok(casinoTexts.some((t) => t.includes("gray")), "灰骰按 gray 展示");
assert.ok(
  casinoLayer.nodes.some((n) => n.t === "rect" && n.stroke === "#e3b341" && n.w === 14),
  "大骰用金框方块标记",
);
const gameLayer = scene.layers.find((l) => l.name === "games")!;
const gameTexts = gameLayer.nodes.filter((n) => n.t === "text").map((n) => (n as { text: string }).text);
assert.ok(gameTexts.some((t) => t.includes("F1")), "小游戏列表显示 id");
assert.ok(gameTexts.some((t) => t.includes("#1 赌场")), "小游戏列表显示赌场号");
assert.ok(casinoTexts.every((t) => !t.includes("F1")), "赌场牌上不再显示小游戏");

// 加密选项不得进版图
assert.ok(!JSON.stringify(scene).includes("K7Q2"), "版图不得出现加密选项代码");

// 跨端契约：仅 6 类原语 + 绝对数字坐标 + 无平台属性
const ALLOWED = new Set(["rect", "circle", "path", "text", "image"]);
const ALLOWED_KEYS: Record<string, Set<string>> = {
  rect: new Set(["t", "x", "y", "w", "h", "rx", "fill", "stroke", "sw", "opacity"]),
  circle: new Set(["t", "cx", "cy", "r", "fill", "stroke", "sw", "opacity"]),
  path: new Set(["t", "d", "fill", "stroke", "sw", "opacity"]),
  text: new Set(["t", "x", "y", "text", "size", "fill", "anchor", "weight", "opacity"]),
  image: new Set(["t", "x", "y", "w", "h", "rx", "href", "opacity"]),
};
const NUM_KEYS = new Set([
  "x", "y", "w", "h", "rx", "cx", "cy", "r", "sw", "size", "weight", "opacity",
]);
for (const layer of scene.layers) {
  for (const n of layer.nodes as SceneNode[]) {
    assert.ok(ALLOWED.has(n.t), `未支持的图元：${n.t}`);
    for (const k of Object.keys(n)) {
      assert.ok(ALLOWED_KEYS[n.t]!.has(k), `图元 ${n.t} 出现平台相关属性：${k}`);
    }
    for (const [k, v] of Object.entries(n)) {
      if (NUM_KEYS.has(k)) {
        assert.ok(typeof v === "number" && Number.isFinite(v), `${n.t}.${k} 必须是有限数字`);
      }
    }
    if (n.t === "text") {
      assert.equal(typeof n.text, "string");
      assert.ok(Array.from(n.text).length <= 60, "文本节点不应过长（防溢出）");
    }
  }
}
assert.ok(scene.assets.every((a) => a.startsWith("/board-assets/")), "素材 URL 走 /board-assets/");

// —— SVG 渲染 ——
const svg = renderBoardSvg(scene);
assert.ok(svg.startsWith("<svg"), "输出 SVG");
assert.ok(svg.includes(`viewBox="0 0 760 ${scene.h}"`), "viewBox 使用场景尺寸");
assert.ok(svg.includes('role="img"'), "有无障碍角色");
assert.ok(svg.includes("&lt;b&gt;机器人1&lt;/b&gt;"), "玩家名应被转义");
assert.ok(!svg.includes("<b>机器人1</b>"), "不得注入原始 HTML");
assert.ok(svg.includes('data-layer="casinos"'), "分层输出");

// —— 空态 / 终局 ——
const empty = buildBoardScene({
  game_id: "las-vegas",
  room_id: "r2",
  openid: "u1",
  public: { status: "active", data: { phase: "lobby", seats: [], casinos: [] } },
  player: { data: {} },
});
const emptySvg = renderBoardSvg(empty);
assert.ok(emptySvg.includes("暂无骰子"), "空桌应有降级文案");
assert.ok(emptySvg.includes("等待开局"), "lobby 阶段文案");

const finished = buildBoardScene({
  ...view,
  public: { status: "finished", data: { ...view.public.data, phase: "finished", winners: ["u2"] } },
});
const finSvg = renderBoardSvg(finished);
assert.ok(finSvg.includes("终局") && finSvg.includes("机器人1"), "终局覆盖层显示胜者");

// 多块小游戏：全部顺序展示（不折叠/不分页）
const manyGames = buildBoardScene({
  ...view,
  public: {
    ...view.public,
    data: {
      ...view.public.data,
      casinos: [
        { dice: {}, banknotes: [0, 0], small_games: ["F1"] },
        { dice: {}, banknotes: [0, 0], small_games: ["H1"] },
        { dice: {}, banknotes: [0, 0], small_games: ["G1"] },
        { dice: {}, banknotes: [0, 0], small_games: ["E1"] },
        { dice: {}, banknotes: [0, 0], small_games: ["C1"] },
        { dice: {}, banknotes: [0, 0], small_games: [] },
      ],
    },
  },
});
const gamesLayer = manyGames.layers.find((l) => l.name === "games")!;
const gameTextsAll = gamesLayer.nodes.filter((n) => n.t === "text").map((n) => (n as { text: string }).text);
for (const id of ["F1", "H1", "G1", "E1", "C1"]) {
  assert.ok(gameTextsAll.some((t) => t.startsWith(id + " ")), `5 块小游戏应全部展示：${id}`);
}
assert.equal(gameTextsAll.filter((t) => t.startsWith("#")).length, 5, "5 块都带赌场号");
assert.ok(manyGames.h > scene.h, "多块小游戏时画布高度自动增长");

// 8 人局：高度增长且不重叠
const many = buildBoardScene({
  ...view,
  public: {
    ...view.public,
    data: {
      ...view.public.data,
      seats: Array.from({ length: 8 }, (_, i) => ({ player_id: `p${i}`, small_remaining: 3 })),
      turn: 7,
    },
  },
});
assert.ok(many.h > scene.h, "8 人局画布更高");

// —— 文本态与双模式页 ——
const textBody = renderGameTextBody(view);
assert.ok(textBody.includes("你的加密选项"), "文本态保留加密选项");

const page = renderAppPage(view, { gameId: "las-vegas", token: "tok-1", mode: "board" });
assert.ok(page.includes('data-view="board"'), "默认版图");
assert.ok(page.includes('id="board-view"') && page.includes('id="text-view"'), "两种视图都在首屏");
assert.ok(page.includes("F1") && page.includes("K7Q2"), "首屏同时含版图与文本内容");
assert.ok(page.includes('data-mode="text"'), "有切换按钮");
assert.ok(page.includes('id="bootstrap"'), "有 bootstrap JSON");
assert.ok(page.includes("/assets/app.js"), "引用客户端 bundle");
assert.ok(page.includes("<noscript>"), "无 JS 降级");

const textPage = renderAppPage(view, { gameId: "las-vegas", token: "tok-1", mode: "text" });
assert.ok(textPage.includes('data-view="text"'), "?view=text 首屏即文本态");

// bootstrap 转义：不得出现未转义的 `</script>`
const evil = renderAppPage(
  { ...view, names: { u1: "</script><script>alert(1)</script>" } },
  { gameId: "las-vegas", token: "tok-1", mode: "board" },
);
assert.ok(!evil.includes("</script><script>alert(1)"), "bootstrap 中的 </script> 必须转义");

console.log("test-view 全部通过 ✅");
