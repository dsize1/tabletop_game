// RN 契约守卫：确保 BoardScene 只使用 react-native-svg 可 1:1 映射的原语，
// 且不含任何 DOM/平台专属属性。RN 端接入前必须保持此测试为绿。
//
// 运行：tsx test-view-rn.mts
import assert from "node:assert";
import { buildBoardScene, type SceneNode } from "./src/view/board/scene.js";
import { renderBoardSvg } from "./src/view/board/svg.js";
import type { GameView } from "./src/view/types.js";

const ALLOWED: Record<string, Set<string>> = {
  rect: new Set(["t", "x", "y", "w", "h", "rx", "fill", "stroke", "sw", "opacity"]),
  circle: new Set(["t", "cx", "cy", "r", "fill", "stroke", "sw", "opacity"]),
  path: new Set(["t", "d", "fill", "stroke", "sw", "opacity"]),
  text: new Set(["t", "x", "y", "text", "size", "fill", "anchor", "weight", "opacity"]),
  image: new Set(["t", "x", "y", "w", "h", "rx", "href", "opacity"]),
};
const NUMERIC = new Set([
  "x", "y", "w", "h", "rx", "cx", "cy", "r", "sw", "size", "weight", "opacity",
]);

/** 覆盖：多块小游戏 + 灰骰 + 终局覆盖层 + 8 人。 */
const rich: GameView = {
  game_id: "las-vegas",
  room_id: "r",
  openid: "p0",
  public: {
    status: "active",
    data: {
      phase: "finished",
      round: 3,
      turn: 7,
      winners: ["p0"],
      seats: Array.from({ length: 8 }, (_, i) => ({
        player_id: `p${i}`,
        small_remaining: 3,
        big_remaining: 1,
        chips: i,
        banknotes: i % 2 ? [30_000] : [],
        is_bot: i > 5,
        markers: i === 0 ? ["grand_prize"] : [],
      })),
      casinos: ["F2", "H1", "G1", "E1", "C1", "H2"].map((id, i) => ({
        dice: { gray: 2, p0: i + 1 },
        big_dice: { p0: i % 2 },
        banknotes: [0, 0],
        small_games: [id],
      })),
      roll: { small: [1, 2, 3], big: 6 },
      sg: {
        f2_slots: [1, null, 2, null, null, 3, null, null, null],
        h1_slots: [{ owner: "p0", dice: 2 }],
        gray_groups: [3, 2],
        lottery: [10_000, 0],
        bars: { p1: { small: 1 } },
        game_field: { p2: 2 },
        gold_frame: { owner: "p3", small: 1, big: 0 },
        banned: 2,
        arrow: 3,
        indicator: 4,
        c1_winner: "p4",
        privilege: "p5",
      },
    },
  },
  player: { data: {} },
  names: Object.fromEntries(Array.from({ length: 8 }, (_, i) => [`p${i}`, `玩家${i}`])),
};

const scene = buildBoardScene(rich);
let nodes = 0;

for (const layer of scene.layers) {
  assert.ok(typeof layer.name === "string" && layer.name.length > 0, "层必须有名字");
  for (const n of layer.nodes as SceneNode[]) {
    nodes += 1;
    const allowedKeys = ALLOWED[n.t];
    assert.ok(allowedKeys, `RN 映射缺原语：${n.t}`);
    for (const k of Object.keys(n)) {
      assert.ok(allowedKeys!.has(k), `原语 ${n.t} 出现 RN 不支持的属性：${k}`);
    }
    for (const [k, v] of Object.entries(n)) {
      if (NUMERIC.has(k)) {
        assert.ok(typeof v === "number" && Number.isFinite(v), `${n.t}.${k} 必须是有限数字`);
      }
    }
    if (n.t === "text") {
      assert.ok(typeof n.text === "string", "text 节点必须有 text");
      assert.ok(n.size > 0 && n.size <= 80, "字号需在合理范围");
      assert.ok(["start", "middle", "end"].includes(n.anchor ?? "start"), "anchor 只能是 start/middle/end");
    }
    if (n.t === "image") {
      assert.ok(n.href.startsWith("/board-assets/") || n.href.startsWith("http"), "image href 必须是可远程加载的 URL");
    }
    if (n.t === "path") {
      assert.ok(/^[MLHVQCZ0-9 ,.\-]+$/i.test(n.d), "path 只允许绝对坐标命令（RN Path 可直接消费）");
      assert.ok(!/[mlhvqc]/.test(n.d), "path 不允许相对命令");
    }
  }
}

// 几何必须落在画布内（RN 端按 viewBox 等比缩放，越界会被裁掉）
for (const layer of scene.layers) {
  for (const n of layer.nodes as SceneNode[]) {
    if (n.t === "rect") {
      assert.ok(n.x >= -20 && n.y >= -20, `rect 越界：${n.x},${n.y}`);
      assert.ok(n.x + n.w <= scene.w + 20, `rect 超宽：${n.x + n.w}`);
      assert.ok(n.y + n.h <= scene.h + 20, `rect 超高：${n.y + n.h}`);
    }
    if (n.t === "circle") {
      assert.ok(n.cx >= -20 && n.cx <= scene.w + 20, `circle 超宽：${n.cx}`);
      assert.ok(n.cy >= -20 && n.cy <= scene.h + 20, `circle 超高：${n.cy}`);
    }
    if (n.t === "image") {
      assert.ok(n.w > 0 && n.h > 0, "image 尺寸必须为正");
    }
  }
}

assert.ok(nodes > 150, `场景图元过少（${nodes}），覆盖不足`);
assert.ok(scene.w === 760 && scene.h > 800, "画布尺寸符合预期");

// 与 Web 渲染共用同一份 scene：SVG 必须能直接产出（渲染层没有额外业务逻辑）
const svg = renderBoardSvg(scene);
assert.ok(svg.includes("<svg") && svg.includes("</svg>"));

console.log(`test-view-rn 全部通过 ✅（${nodes} 个图元，5 类原语）`);
