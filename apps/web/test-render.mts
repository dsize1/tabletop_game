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
        { dice: { u1: 3, gray: 2 }, banknotes: [0, 0], small_games: ["C1"] },
        { dice: {}, banknotes: [0, 0], small_games: [] },
      ],
    },
  },
  player: { data: {} },
};
const gameHtml = renderGamePage(view);
assert.ok(gameHtml.includes("拉斯维加斯"), "牌面页应含标题");
assert.ok(gameHtml.includes("第 2 轮"), "牌面页应含轮次");
assert.ok(gameHtml.includes("（我）"), "牌面页应标记本人");
assert.ok(gameHtml.includes("C1"), "牌面页应含小游戏");
assert.ok(gameHtml.includes("1 张（3 万）"), "个人资源应含已得钞票金额");
assert.ok(gameHtml.includes("⏳"), "应标记当前回合玩家");

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
