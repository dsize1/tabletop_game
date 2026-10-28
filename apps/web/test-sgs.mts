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
import {
  isBlockingPending,
  overlayKindFor,
  renderChooseManyOverlay,
  renderDyingOverlay,
  renderPickCardOverlay,
  renderTargetOverlay,
  renderWuguOverlay,
  renderWuxieOverlay,
} from "./src/view/sgs/pending.js";
import { classifyPress, MOVE_TOLERANCE_PX, LONG_PRESS_MS } from "./src/client/sgs-gestures.js";
import { renderSgsPage, SGS_STYLES } from "./src/view/sgs/page.js";
import {
  GX_SLOT_IDS,
  applyDrop,
  applyMoveWithinColumn,
  applyTap,
  arrangeAction,
  firstEmpty,
  initialSlots,
  renderGuanxingOverlay,
  settle,
  snapSlot,
  snapThreshold,
} from "./src/view/sgs/guanxing.js";
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
// 观星自 P3.5 起只在 pending 浮层里渲染（个人区不再重复占位）
assert.ok(!personalWithZones.includes('data-zone="guanxing"'), "观星改由浮层承载");
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

// —— P3.4 pending 浮层 ——
const wuxiePending = {
  kind: "wuxie",
  actor: "p0",
  options: [
    { id: "respond:c9", label: "打出【无懈可击♣Q】" },
    { id: "pass", label: "不出" },
  ],
  data: { root: "【南蛮入侵】→ 甘宁", layers: 2, count: 2 },
};
const wuxieHtml = renderWuxieOverlay(wuxiePending, board, names);
assert.ok(wuxieHtml.includes("【南蛮入侵】→ 甘宁"), "无懈链展示 root");
assert.ok(wuxieHtml.includes("已打出无懈：2 层"), "无懈链层数");
assert.ok(wuxieHtml.includes('data-action="respond:c9"') && wuxieHtml.includes('data-action="pass"'), "无懈链动作按钮");
assert.ok(wuxieHtml.includes("必须显式选择"), "无懈链提示不可误关");

const dyingHtml = renderDyingOverlay(
  { kind: "dying", actor: "p0", options: [{ id: "respond:c2", label: "使用桃救援" }, { id: "pass", label: "不出" }], data: { victim: "p3", hp: -1 } },
  board,
  names,
);
assert.ok(dyingHtml.includes("濒死") && dyingHtml.includes("体力 -1"), "濒死浮层展示受害者与体力");
assert.ok(dyingHtml.includes('data-action="respond:c2"'), "濒死出桃按钮");

const zonesWugu = {
  wugu: {
    visibility: "public",
    cards: [card("c5", "杀", "heart", 5), card("c6", "闪", "diamond", 8, "shan")],
    choosers: ["p0", "p1"],
    cursor: 0,
    current: "p0",
    picked: [{ player: "p1", card: card("c4", "桃", "heart", 4, "tao") }],
  },
};
const wuguHtml = renderWuguOverlay(
  { kind: "wugu", actor: "p0", options: [{ id: "pick:c5", label: "杀♥5" }, { id: "pick:c6", label: "闪♦8" }], data: { chooser: "p0" } },
  board,
  zonesWugu,
  names,
);
assert.ok(wuguHtml.includes('data-action="pick:c5"'), "五谷选牌按钮");
assert.ok(wuguHtml.includes("李四：桃♥4"), "五谷已选列表");
assert.ok(wuguHtml.includes("当前选择"), "五谷当前选择者");

const pickHtml = renderPickCardOverlay(
  {
    kind: "pick_card",
    actor: "p0",
    options: [
      { id: "pick:equip:weapon", label: "装备区：青釭剑♠6" },
      { id: "pick:hand", label: "手牌：随机 1 张（共 3 张）" },
      { id: "pick:judge:c8", label: "判定区：乐不思蜀♣6" },
    ],
    data: { card: "c1", target: "p1" },
  },
  board,
  names,
);
assert.ok(pickHtml.includes("装备区") && pickHtml.includes("手牌（随机 1 张）") && pickHtml.includes("判定区"), "拿牌三组");
assert.ok(pickHtml.includes('data-action="pick:hand"'), "随机手牌按钮");
assert.ok(pickHtml.includes("曹操"), "展示目标");

const targetHtml = renderTargetOverlay(
  { kind: "target", actor: "p0", options: [{ id: "target:p1", label: "对 李四 使用【杀】" }], data: { card: "c1", stage: "single" } },
  board,
  names,
  () => "杀♠7",
);
assert.ok(targetHtml.includes("杀♠7") && targetHtml.includes('data-action="target:p1"'), "目标选择按钮");
const borrowHtml = renderTargetOverlay(
  { kind: "target", actor: "p0", options: [{ id: "target:p1", label: "借 李四 的刀" }], data: { stage: "borrow_first" } },
  board,
  names,
);
assert.ok(borrowHtml.includes("借谁的刀"), "借刀第一阶段提示");

const manyHtml = renderChooseManyOverlay(
  {
    kind: "choose_many",
    actor: "p0",
    skill: "zhiheng",
    options: [
      { id: "choose:c1", label: "杀♠7" },
      { id: "confirm", label: "确认" },
      { id: "cancel", label: "取消" },
    ],
    data: { purpose: "zhiheng", selected: ["c1"], min: 1, max: 2 },
  },
  me,
  names,
);
assert.ok(manyHtml.includes("已选 1 张（1–2）"), "多选计数");
assert.ok(manyHtml.includes('data-action="choose:c1"') && manyHtml.includes('data-action="confirm"'), "多选按钮");
assert.ok(manyHtml.includes("sgs-multi-card on"), "已选牌高亮");

assert.equal(overlayKindFor({ kind: "target" } as never), "target");
assert.equal(overlayKindFor({ kind: "discard" } as never), null, "弃牌留在个人区");
assert.ok(isBlockingPending("wuxie") && isBlockingPending("dying") && isBlockingPending("wugu"), "无懈/濒死/五谷不可误关");
assert.ok(!isBlockingPending("target"), "目标选择可关闭（个人区仍可继续）");
const blockingShell = renderOverlayShell("无懈可击", "x", { blocking: true });
assert.ok(!blockingShell.includes('data-overlay-close="button"'), "阻塞浮层无关闭按钮");
assert.ok(blockingShell.includes("sgs-overlay-lock"), "阻塞浮层显示「需选择」");

console.log("[sgs-web] 特殊区 pending 浮层单测通过 ✅");

// —— P3.5 观星 2×5 ——
const gx0 = initialSlots(["c3", "c1", "c2"]);
assert.deepEqual(
  GX_SLOT_IDS,
  ["L1", "L2", "L3", "L4", "L5", "R1", "R2", "R3", "R4", "R5"],
  "槽位恒为 10",
);
assert.deepEqual([gx0.L1, gx0.L2, gx0.L3, gx0.L4, gx0.R1], ["c3", "c1", "c2", null, null], "初始 L1..Lk");
assert.equal(firstEmpty(gx0, "L"), "L4");
assert.equal(firstEmpty(gx0, "R"), "R1");

// 短按：跨列首个空槽
const tap1 = applyTap(gx0, "L1");
assert.equal(tap1.L1, null);
assert.equal(tap1.R1, "c3", "短按移入另一列首个空槽");
assert.deepEqual(settle(tap1), ["c1", "c2", "c3"], "跳空压缩：左列接右列");

// 同列上移/下移（与相邻槽交换）
const moved = applyMoveWithinColumn(gx0, "L3", -1);
assert.deepEqual([moved.L2, moved.L3], ["c2", "c1"], "上移=与上一槽交换");
assert.deepEqual(applyMoveWithinColumn(gx0, "L1", -1), gx0, "越界不移动");
assert.deepEqual(applyMoveWithinColumn(gx0, "L1", 1).L1, "c1", "下移后原位置拿到下方牌");

// 拖拽：空槽放入 / 有牌交换 / 同槽或超距回原位
const dropEmpty = applyDrop(gx0, "L1", "R2");
assert.equal(dropEmpty.R2, "c3");
assert.equal(dropEmpty.L1, null);
const dropSwap = applyDrop(gx0, "L1", "L2");
assert.deepEqual([dropSwap.L1, dropSwap.L2], ["c1", "c3"], "有牌槽交换");
assert.deepEqual(applyDrop(gx0, "L1", null), gx0, "超距回原位");
assert.deepEqual(applyDrop(gx0, "L1", "L1"), gx0, "落回原槽无操作");
assert.deepEqual(applyDrop(gx0, "L4", "R1"), gx0, "空槽不可拖动");

// 吸附：阈值 = 槽短边 10%
const rect = { left: 0, top: 0, width: 80, height: 40 };
assert.equal(snapThreshold(rect), 4, "阈值取短边 10%");
const rects = { L1: rect, L2: { left: 0, top: 50, width: 80, height: 40 } };
assert.equal(snapSlot({ x: 40, y: 20 }, rects, 4), "L1", "中心吸附");
assert.equal(snapSlot({ x: 40, y: 71 }, rects, 4), "L2", "就近吸附（L2 中心 ±4px 内）");
assert.equal(snapSlot({ x: 40, y: 35 }, rects, 4), null, "超出阈值返回 null");

// 提交 id
assert.equal(arrangeAction(tap1), "arrange:c1,c2,c3");
assert.equal(arrangeAction(initialSlots([])), "arrange:");

// 渲染
const gxCards = [
  { id: "c3", def: "sha", name: "杀", suit: "spade", rank: 3 },
  { id: "c1", def: "shan", name: "闪", suit: "heart", rank: 1 },
  { id: "c2", def: "tao", name: "桃", suit: "diamond", rank: 2 },
] as const;
const gxHtml = renderGuanxingOverlay(gx0, gxCards as never);
assert.ok(gxHtml.includes('data-gx-slot="L1"') && gxHtml.includes('data-gx-card="c3"'), "槽位渲染");
assert.ok(gxHtml.includes("♠3") && gxHtml.includes("杀"), "牌面渲染");
assert.ok(gxHtml.includes('data-gx-move="down"'), "下移按钮");
assert.ok(gxHtml.includes("data-gx-reset"), "重置按钮");
assert.ok(gxHtml.includes('data-action="arrange:c3,c1,c2"'), "提交动作 id");
assert.ok(gxHtml.includes("最终顺序（顶 → 底）：杀♠3 → 闪♥A → 桃♦2"), "顺序预览");

console.log("[sgs-web] 观星纯函数/渲染单测通过 ✅");
