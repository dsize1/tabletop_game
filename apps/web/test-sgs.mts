// 三国杀视图层纯函数单测（无需起服务）。
// 运行：cd apps/web && npx tsx test-sgs.mts
import assert from "node:assert";
import {
  cardLabel,
  judgeIcon,
  othersInSeatOrder,
  rankLabel,
  seatPlacements,
  splitSides,
} from "./src/view/sgs/format.js";
import { renderPublicArea, renderSidePanel } from "./src/view/sgs/public.js";
import { renderPersonalArea } from "./src/view/sgs/personal.js";
import { renderSgsTextBody } from "./src/view/sgs/text.js";
import { renderCardDetail, renderDiscardList, renderGeneralDetail } from "./src/view/sgs/detail.js";
import { renderOverlayShell } from "./src/view/sgs/overlay.js";
import { classifyPress, MOVE_TOLERANCE_PX, LONG_PRESS_MS } from "./src/client/sgs-gestures.js";
import { renderSgsPage, SGS_STYLES } from "./src/view/sgs/page.js";
import type { SgsCard, SgsPlayerArea, SgsPublicBoard, SgsViewData } from "./src/view/sgs/types.js";
import type { GameView } from "./src/view/types.js";

const card = (id: string, name: string, suit: SgsCard["suit"], rank: number, def = "sha"): SgsCard => ({
  id,
  def,
  name,
  suit,
  rank,
});

const seats: SgsPublicBoard["seats"] = [
  0, 1, 2, 3, 4,
].map((i) => ({
  seat: i,
  player_id: `p${i}`,
  identity: i === 0 ? "lord" : null,
  general: `sgs_hero_${i}`,
  general_name: ["刘备", "曹操", "孙权", "貂蝉", "吕布"][i],
  kingdom: ["shu", "wei", "wu", "qun", "qun"][i],
  hp: 3,
  max_hp: 4,
  hand_count: i + 1,
  equips: { weapon: i === 1 ? card("c9", "青釭剑", "spade", 6, "qinggang_jian") : null },
  judge_zone: i === 2 ? [card("c8", "乐不思蜀", "club", 6, "lebu_sishu")] : [],
  alive: i !== 4,
}));

const board: SgsPublicBoard = {
  phase: "turn",
  round: 2,
  turn_index: 7,
  turn: { seat: 0, phase: "play", player_id: "p0" },
  seat_count: 5,
  seats,
  deck: { draw: 60, discard: 12, processing: 0, void: 0 },
  discard_top: card("c7", "杀", "spade", 7),
  pending: { kind: "wuxie", actor: "p1" },
  history: [{ seq: 1, turn: 0, text: "刘备 使用【杀♠7】" }],
};

const me: SgsPlayerArea = {
  seat: 0,
  player_id: "p0",
  identity: "lord",
  general: "sgs_hero_1",
  general_name: "刘备",
  kingdom: "shu",
  hp: 4,
  max_hp: 5,
  hand: [card("c1", "杀", "spade", 7), card("c2", "桃", "heart", 3, "tao")],
  equips: { weapon: null, armor: null, horse_offense: null, horse_defense: null, treasure: null },
  judge_zone: [],
  skills: [
    { code: "rende", name: "仁德", type: "normal", implemented: true },
    { code: "jijiang", name: "激将", type: "lord", implemented: true },
  ],
  is_current: true,
  pending: null,
};

const view: SgsViewData = {
  schema_version: 2,
  game: "sanguosha",
  phase: "turn",
  public_board: board,
  player_area: me,
  special_zones: {},
};

const names = { p0: "张三", p1: "李四" };

// —— 座位布局（ui-spec §3.2 公式）——
assert.deepEqual(splitSides(4), { left: 1, top: 2, right: 1 }, "5 人局 1/2/1");
assert.deepEqual(splitSides(5), { left: 1, top: 3, right: 1 }, "6 人局 1/3/1");
assert.deepEqual(splitSides(6), { left: 2, top: 3, right: 1 }, "7 人局 2/3/1");
assert.deepEqual(splitSides(7), { left: 2, top: 3, right: 2 }, "8 人局 2/3/2");
const placements = seatPlacements(7);
assert.equal(placements.length, 7, "座位数应等于他人数");
assert.deepEqual(
  placements.map((p) => p.index),
  [0, 1, 2, 3, 4, 5, 6],
  "填位顺序：左列→上排→右列",
);
assert.deepEqual(
  placements.map((p) => p.side),
  ["left", "left", "top", "top", "top", "right", "right"],
  "8 人局左2/上3/右2",
);
assert.ok(placements[0].top > placements[1].top, "左列自下而上");
assert.deepEqual(othersInSeatOrder(5, 3), [4, 0, 1, 2], "左手侧顺序");

// —— 牌面/文案 ——
assert.equal(rankLabel(1), "A");
assert.equal(rankLabel(11), "J");
assert.equal(judgeIcon(card("x", "乐不思蜀", "club", 6, "lebu_sishu")), "乐");
assert.equal(judgeIcon(card("y", "闪电", "spade", 1, "shandian")), "雷");
assert.equal(cardLabel(card("c1", "杀", "spade", 7)), "杀♠7");

// —— 公共区：只读、无手牌明细、含光环信息 ——
const publicHtml = renderPublicArea(view, 0, names);
assert.ok(publicHtml.includes("李四"), "应显示其他玩家昵称");
assert.ok(publicHtml.includes("曹操") && publicHtml.includes("吕布"), "应显示其他玩家武将名");
assert.ok(publicHtml.includes("×2"), "应显示手牌数量");
assert.ok(publicHtml.includes("乐"), "判定区应有一字图标");
assert.ok(publicHtml.includes("青釭剑"), "装备应可见");
assert.ok(!publicHtml.includes("sgs-card-rank"), "公共区不得出现手牌正面");
assert.ok(!publicHtml.includes("桃♥3"), "公共区不得泄露他人手牌");
assert.ok(publicHtml.includes("等待 李四 响应"), "应提示当前等待者");
assert.ok(publicHtml.includes("第 2 轮"), "应显示轮次");
const sideHtml = renderSidePanel(view, names);
assert.ok(sideHtml.includes("出牌历史") && sideHtml.includes("使用【杀♠7】"), "辅助区应含历史");

// —— 个人区：手牌正面 / 技能 / 操作条 / pending ——
const meHtml = renderPersonalArea(me, board, {}, names);
assert.ok(meHtml.includes("杀♠7") && meHtml.includes("桃♥3"), "个人区应显示手牌正面");
assert.ok(meHtml.includes('data-skill="rende"'), "技能按钮");
assert.ok(meHtml.includes("确认打出") && meHtml.includes("disabled"), "未选中时确认按钮置灰");
assert.ok(meHtml.includes('data-action="end_play"'), "结束出牌按钮");
const withPending: SgsPlayerArea = {
  ...me,
  pending: {
    kind: "discard",
    actor: "p0",
    options: [
      { id: "discard:c1", label: "杀♠7" },
      { id: "discard:c2", label: "桃♥3" },
    ],
    data: { required: 2, reason: "discard_phase" },
  },
};
assert.ok(
  renderPersonalArea(withPending, board, {}, names).includes("需弃 2 张"),
  "弃牌 pending 应显示张数",
);

// —— 特殊区：观星（owner）+ 五谷（public）——
const zones: SgsViewData["special_zones"] = {
  guanxing: {
    owner: "p0",
    visibility: "owner",
    rows: 5,
    cols: 2,
    slots: { L1: "c3", L2: "c1", L3: null, L4: null, L5: null, R1: null, R2: null, R3: null, R4: null, R5: null },
    rev: 1,
    interaction: { kind: "ordered", commit: "arrange" },
  },
  wugu: {
    visibility: "public",
    cards: [card("c5", "杀", "heart", 5), card("c6", "闪", "diamond", 8, "shan")],
    choosers: ["p0", "p1"],
    cursor: 1,
    current: "p1",
    picked: [{ player: "p0", card: card("c4", "桃", "heart", 4, "tao") }],
    interaction: { kind: "one", commit: "pick" },
  },
};
const personalWithZones = renderPersonalArea(me, board, zones, names);
assert.ok(personalWithZones.includes('data-zone="guanxing"'), "观星区渲染");
assert.ok(personalWithZones.includes('data-slot="L1"'), "观星槽位 id");
assert.ok(personalWithZones.includes('data-zone="wugu"') && personalWithZones.includes('data-pick="c5"'), "五谷选牌按钮");
assert.ok(personalWithZones.includes("五谷丰登 · 当前 李四"), "五谷当前选择者");

// —— 文本态（无 JS）：可读 + 表单提交 + 观星文字顺序 ——
const textView: SgsViewData = { ...view, special_zones: zones };
const textHtml = renderSgsTextBody(textView, names, {
  gameId: "sanguosha",
  token: "tok-1",
  roomId: "room-9",
});
assert.ok(textHtml.includes("公共局面") && textHtml.includes("我的资源"), "文本态应含公共/个人");
assert.ok(textHtml.includes("杀♠7"), "文本态手牌可读");
assert.ok(textHtml.includes("左列：c3 → c1"), "观星文字顺序");
assert.ok(textHtml.includes("<code>arrange:"), "观星文字提交格式");
assert.ok(textHtml.includes('method="post"') && textHtml.includes('action="/tabletop_game/sanguosha/action"'), "动作表单");
assert.ok(textHtml.includes('name="token" value="tok-1"'), "表单携带 token");
assert.ok(textHtml.includes('name="room_id" value="room-9"'), "表单携带 room_id");

// —— 整页：bootstrap / bundle / noscript / 禁用特性 ——
const gv: GameView = {
  game_id: "sanguosha",
  room_id: "room-9",
  openid: "p0",
  public: { data: view as unknown as GameView["public"]["data"] },
  player: { data: view as unknown as GameView["public"]["data"] },
  names,
};
const pageHtml = renderSgsPage(gv, {
  gameId: "sanguosha",
  token: "tok-1",
  openid: "p0",
  roomId: "room-9",
});
assert.ok(pageHtml.includes('id="sgs-bootstrap"'), "bootstrap JSON");
assert.ok(pageHtml.includes('src="/assets/sgs.js"'), "客户端 bundle");
assert.ok(pageHtml.includes("<noscript>"), "无 JS 降级");
assert.ok(pageHtml.includes("viewport-fit=cover"), "安全区适配");
assert.ok(!pageHtml.includes("localStorage"), "token 不落 localStorage");
assert.ok(!SGS_STYLES.includes(":has("), "禁用 :has()");
assert.ok(!SGS_STYLES.includes("@container"), "禁用容器查询");
assert.ok(!/\b[0-9]+dvh\b/.test(SGS_STYLES), "禁用 dvh");

// —— XSS：昵称必须转义 ——
const xss = renderPublicArea(view, 0, { ...names, p1: '<img src=x onerror=alert(1)>' });
assert.ok(!xss.includes("<img src=x"), "昵称必须转义");
assert.ok(xss.includes("&lt;img"), "转义后保留文本");

// —— 空个人区（未坐下/阵亡）降级 ——
assert.ok(
  renderPersonalArea(null, board, {}, names).includes("个人资源区不可见"),
  "缺少 player_area 时应降级",
);

console.log("[sgs-web] 视图层单测通过 ✅（座位布局/牌面/公共区/个人区/特殊区/文本态/整页）");

// —— 手势判定（ui-spec §5：<400ms 短按；≥400ms 长按；>8px 取消）——
const press = (downAt: number, upAt: number, dx = 0, dy = 0, extra: Record<string, boolean> = {}) =>
  classifyPress({ downAt, upAt, dx, dy, ...extra });
assert.equal(press(0, 399), "tap", "399ms = 短按");
assert.equal(press(0, LONG_PRESS_MS), "longpress", "≥400ms = 长按");
assert.equal(press(0, 120, MOVE_TOLERANCE_PX, 0), "tap", "位移 8px 仍算点击");
assert.equal(press(0, 120, MOVE_TOLERANCE_PX + 1, 0), "move", "位移 >8px = 滚动意图");
assert.equal(press(0, 120, 6, 6), "move", "斜向位移按欧氏距离（6,6 → 8.49px > 8）");
assert.equal(press(0, 500, 0, 0, { cancelled: true }), "cancel", "pointercancel 不产生点击");
assert.equal(press(0, 500, 0, 0, { longFired: true }), "longpress", "长按到时后 up 不再触发点击");

// —— 详情浮层内容 ——
const detailCard = renderCardDetail(card("c1", "青釭剑", "spade", 6, "qinggang_jian"));
assert.ok(detailCard.includes("青釭剑") && detailCard.includes("♠6"), "牌面详情含花色点数");
const generalDetail = renderGeneralDetail(seats[1], names);
assert.ok(generalDetail.includes("曹操") && generalDetail.includes("未公开"), "武将详情（非主公身份不公开）");
const discardHtml = renderDiscardList({
  ...board,
  discard: [card("c1", "杀", "spade", 7), card("c2", "闪", "heart", 2, "shan")],
});
assert.ok(discardHtml.includes("共 2 张") && discardHtml.includes("闪♥2"), "弃牌堆列表");
assert.ok(discardHtml.indexOf("闪♥2") < discardHtml.indexOf("杀♠7"), "弃牌堆顶在前");
assert.ok(renderDiscardList({ ...board, discard: [] }).includes("弃牌堆为空"), "空弃牌堆");

// —— 浮层外壳 ——
const shell = renderOverlayShell("牌面详情", "<b>内容</b>");
assert.ok(shell.includes("sgs-overlay-mask") && shell.includes("data-overlay-close"), "外壳含遮罩与关闭");
assert.ok(shell.includes("<h2>牌面详情</h2>") && shell.includes("<b>内容</b>"), "外壳含标题与内容");
const shellXss = renderOverlayShell("<img src=x>", "");
assert.ok(!shellXss.includes("<img src=x"), "标题转义");
const shellWide = renderOverlayShell("观星", "", { size: "wide" });
assert.ok(shellWide.includes("sgs-overlay-card wide"), "wide 变体");

// —— data-press 钩子（手势委托依赖）——
assert.ok(publicHtml.includes('data-press="general"'), "武将卡可点开详情");
assert.ok(publicHtml.includes('data-press="discard"'), "弃牌堆可点开列表");
assert.ok(meHtml.includes('data-press="hand"'), "手牌可手势操作");
const meWithEquip = renderPersonalArea(
  { ...me, equips: { ...me.equips, weapon: card("c9", "青釭剑", "spade", 6, "qinggang_jian") } },
  board,
  {},
  names,
);
assert.ok(meWithEquip.includes('data-press="card"'), "装备/判定小标记可长按详情");
assert.ok(publicHtml.includes('data-press="card"'), "他人装备/判定小标记可长按详情");

console.log("[sgs-web] 手势/浮层/详情单测通过 ✅");

// —— P3.3 出牌链路：actions 驱动可用性 + pending 动作按钮 ——
const actions = [
  { id: "play:c1", description: "使用【杀♠7】" },
  { id: "play:c2", description: "使用【桃♥3】" },
  { id: "end_play", description: "结束出牌阶段" },
];
const withActions = renderPersonalArea(me, board, {}, names, actions);
assert.ok(withActions.includes('class="sgs-card red playable"') || withActions.includes("playable"), "可出的牌应标记 playable");
assert.ok(withActions.includes('data-action="end_play"'), "结束出牌按钮携带动作 id");
assert.ok(!/data-action="end_play"[^>]*disabled/.test(withActions), "有 end_play 动作时应启用");
const noActions = renderPersonalArea(me, board, {}, names, []);
assert.ok(/data-action="end_play"[^>]*disabled/.test(noActions), "无动作时结束出牌置灰");

const pendingButtons = renderPersonalArea(
  {
    ...me,
    pending: {
      kind: "respond",
      actor: "p0",
      options: [
        { id: "respond:c2", label: "打出【桃♥3】" },
        { id: "pass", label: "不出" },
      ],
    },
  },
  board,
  {},
  names,
  [],
);
assert.ok(pendingButtons.includes('data-action="respond:c2"'), "pending 选项可提交");
assert.ok(pendingButtons.includes('data-action="pass"'), "显式不出按钮");

const skillActions = renderPersonalArea(me, board, {}, names, [
  { id: "skill:rende", description: "仁德" },
]);
assert.ok(
  /data-skill="rende"[^>]*data-action="skill:rende"/.test(skillActions),
  "有主动技动作时技能按钮可点",
);
const noSkillAction = renderPersonalArea(me, board, {}, names, []);
assert.ok(/data-skill="rende"[^>]*disabled/.test(noSkillAction), "无对应动作时技能按钮置灰");

console.log("[sgs-web] 出牌链路（可用性/动作按钮）单测通过 ✅");
