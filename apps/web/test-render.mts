// web 文档树渲染纯函数单测（无需起服务）。
// 运行：npx tsx test-render.mts
import assert from "node:assert";
import { renderDocsTree, renderGamePage, type GameView } from "./src/render.js";

const roots = [
  {
    name: "docs",
    path: "docs",
    children: [
      {
        name: "conventions",
        type: "dir" as const,
        path: "conventions",
        children: [
          {
            name: "mcp-primitives.md",
            type: "file" as const,
            path: "conventions/mcp-primitives.md",
            md: true,
          },
        ],
      },
      { name: "notes.txt", type: "file" as const, path: "notes.txt", md: false },
    ],
  },
];

const html = renderDocsTree(roots, "tok-1");
assert.ok(html.includes('<details class="root" open><summary>docs</summary>'), "根节点应为打开的 <details>");
assert.ok(html.includes('<details class="dir"><summary>conventions</summary>'), "目录应为可折叠 <details>");
assert.ok(
  html.includes("/dev_docs/file?token=tok-1&root=docs&path=conventions%2Fmcp-primitives.md"),
  ".md 链接应携带 token + 编码后的 root/path",
);
assert.ok(html.includes('class="file muted">notes.txt'), "非 .md 应灰显且不可点");
assert.ok(!html.includes('href="/dev_docs/file?token=tok-1&root=docs&path=notes.txt"'), "非 .md 不应生成链接");

// 名称转义（防 XSS）
const xss = renderDocsTree(
  [
    {
      name: "x",
      path: "x",
      children: [{ name: "<img src=x onerror=alert(1)>", type: "file" as const, path: "a.md", md: false }],
    },
  ],
  "t",
);
assert.ok(xss.includes("&lt;img") && !xss.includes("<img"), "文件名应被 HTML 转义");

// 空根
assert.ok(renderDocsTree([], "t").includes("暂无可用文档根"), "无根时应给出提示");

// —— 拉斯维加斯牌面页 ——
const view: GameView = {
  game_id: "las-vegas",
  room_id: "r1",
  openid: "u1",
  public: {
    status: "active",
    data: {
      phase: "turn_roll",
      round: 2,
      turn: 1,
      start_player: "u1",
      seats: [
        { player_id: "u1", small_remaining: 3, big_remaining: 1, chips: 2, banknotes: [30_000] },
        { player_id: "u2", small_remaining: 0, big_remaining: 0, chips: 4, banknotes: [] },
      ],
      casinos: [
        { dice: { u1: 3, gray: 2 }, big_dice: { u1: 1 }, banknotes: [0, 0], small_games: ["C1", "F2"] },
        { dice: {}, banknotes: [0, 0], small_games: ["H1", "G2", "H2"] },
      ],
      sg: {
        f2_slots: [1, null, 2, null, null, 3, null, null, null],
        h1_slots: [{ owner: "u1", dice: 2, big: 0 }, null, null, null, null, null],
        game_field: { u2: 2 },
        gold_frame: { owner: "u2", small: 1, big: 0 },
        bars: { u1: { small: 1, big: 0 } },
        gray_groups: [3, 2, 2],
        lottery: [0, 0],
        indicator: 2,
        banned: null,
        arrow: 1,
        c1_winner: "u1",
        privilege: "u2",
      },
    },
  },
  player: {
    data: {
      sg: {
        pending_options: {
          game: "A1",
          stage: "hide",
          multi: false,
          options: [
            { code: "K7Q2", count: 1, reward: "2 筹码" },
            { code: "9FX3", count: 2, reward: "$3万" },
            { code: "M4AB", count: 3, reward: "$4万" },
          ],
        },
      },
    },
  },
  names: { u1: "轮子总比困难多", u2: "机器人1" },
};
const gameHtml = renderGamePage(view);
assert.ok(gameHtml.includes("拉斯维加斯"), "牌面页应含标题");
assert.ok(gameHtml.includes("第 2 轮"), "牌面页应含轮次");
assert.ok(gameHtml.includes("（我）"), "牌面页应标记本人");
assert.ok(gameHtml.includes("C1"), "牌面页应含小游戏");
assert.ok(gameHtml.includes("含大骰 1"), "应标出等效骰数中的大骰颗数");
assert.ok(gameHtml.includes("小游戏状态"), "应含小游戏状态模块");
assert.ok(gameHtml.includes("你的加密选项"), "个人视图应渲染加密选项");
assert.ok(gameHtml.includes("K7Q2") && gameHtml.includes("藏 2 颗标记"), "应显示代码与含义");
assert.ok(gameHtml.includes("#选 K7Q2"), "应提示用代码回复");
assert.ok(gameHtml.includes("危机就是转机"), "F2 应显示名称");
assert.ok(gameHtml.includes("（1/2）") && gameHtml.includes("（1/3）") && gameHtml.includes("（1/4）"), "F2 应显示各组占格数");
assert.ok(gameHtml.includes("场上灰骰：1 号×2"), "F2 应显示场上灰骰分布");
assert.ok(gameHtml.includes("好骰难求") && gameHtml.includes("轮子总比困难多×2"), "H1 应显示格位占用");
assert.ok(!gameHtml.includes("游戏场空") && gameHtml.includes("机器人1×2"), "G2 游戏场应显示");
assert.ok(gameHtml.includes("金框：机器人1（小×1）"), "H2 金框应显示");
assert.ok(gameHtml.includes("1 张（3 万）"), "个人资源应含已得钞票金额");
assert.ok(gameHtml.includes("⏳"), "应标记当前回合玩家");
assert.ok(gameHtml.includes("轮子总比困难多"), "应显示群昵称而非 openid");
assert.ok(gameHtml.includes("机器人1"), "应显示机器人昵称");
assert.ok(!gameHtml.includes(">u1（我）<" ), "不应直接用 openid 作显示名");

// 终局页
const winHtml = renderGamePage({
  ...view,
  public: { status: "finished", data: { phase: "finished", winners: ["u2"], seats: [], casinos: [] } },
});
assert.ok(winHtml.includes("胜者") && winHtml.includes("u2"), "终局应显示胜者");

// XSS：玩家名注入应被转义
const xssGame = renderGamePage({
  ...view,
  public: {
    status: "active",
    data: { phase: "turn_roll", seats: [{ player_id: "<img src=x>" }], casinos: [] },
  },
});
assert.ok(xssGame.includes("&lt;img") && !xssGame.includes("<img"), "玩家名应被 HTML 转义");

console.log("test-render 全部通过 ✅");
