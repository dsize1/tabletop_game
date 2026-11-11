// 三国杀视图层纯函数单测（无需起服务）。
// 运行：cd apps/web && npx tsx test-sgs.mts
import assert from "node:assert";
import {
  applyNames,
  cardLabel,
  judgeIcon,
  othersInSeatOrder,
  rankLabel,
  seatPlacements,
  splitSides,
  withDisplayNames,
} from "./src/view/sgs/format.js";
import { renderPublicArea, renderSidePanel, renderSidePanelBody } from "./src/view/sgs/public.js";
import { renderPersonalArea } from "./src/view/sgs/personal.js";
import { renderSgsTextBody } from "./src/view/sgs/text.js";
import { renderCardDetail, renderDiscardList, renderGeneralDetail } from "./src/view/sgs/detail.js";
import { renderOverlayShell } from "./src/view/sgs/overlay.js";
import {
  isBlockingPending,
  overlayKindFor,
  renderChooseManyOverlay,
  renderCollateralOverlay,
  renderDyingOverlay,
  renderFireAttackOverlay,
  renderPickCardOverlay,
  renderPindianOverlay,
  renderQinwangOverlay,
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
  draftFromZone,
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
  hand_limit: 4,
  equips: { weapon: i === 1 ? card("c9", "青釭剑", "spade", 6, "qinggang_jian") : null },
  judge_zone: i === 2 ? [card("c8", "乐不思蜀", "club", 6, "lebu_sishu")] : [],
  alive: i !== 4,
}));

seats[4].hand_limit = null;

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
  history: [
    { seq: 1, turn: 0, text: "刘备 使用【杀♠7】" },
    { seq: 2, turn: 0, actor: "p0", text: "受到 1 点普通伤害（体力 3）" },
    { seq: 3, turn: 0, text: "【杀】结算完毕" },
  ],
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
  hand_limit: 5,
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
assert.ok(publicHtml.includes("×2/4"), "应显示手牌数量/上限");
assert.ok(publicHtml.includes("×5/∞"), "无上限（null）时显示 ∞");
assert.ok(publicHtml.includes("乐"), "判定区应有一字图标");
assert.ok(publicHtml.includes("青釭剑"), "装备应可见");
assert.ok(!publicHtml.includes("sgs-card-rank"), "公共区不得出现手牌正面");
assert.ok(!publicHtml.includes("桃♥3"), "公共区不得泄露他人手牌");
assert.ok(publicHtml.includes("等待 李四 响应"), "应提示当前等待者");
assert.ok(publicHtml.includes("第 2 轮"), "应显示轮次");
const sideHtml = renderSidePanel(view, names);
assert.ok(sideHtml.includes('id="sgs-side"'), "侧栏有稳定 id（客户端局部刷新）");
assert.ok(sideHtml.includes("出牌历史") && sideHtml.includes("使用【杀♠7】"), "辅助区应含历史");
assert.ok(sideHtml.includes("出牌历史（3）"), "历史标题显示总条数（全量）");
assert.ok(
  sideHtml.includes('class="sgs-log-actor">张三</b> 受到 1 点普通伤害'),
  "历史条目显示操作者昵称（便于复盘）",
);
const seqs = [...sideHtml.matchAll(/data-seq="(\d+)"/g)].map((m) => Number(m[1]));
assert.deepEqual(seqs, [3, 2, 1], "历史倒序（最新在最上面）");
const sideBody = renderSidePanelBody(view, names);
assert.ok(!sideBody.includes("<aside"), "body 渲染器不带外壳（仅替换 innerHTML）");
assert.ok(sideBody.includes("出牌历史"), "body 含历史");

// 他人手牌堆叠：最多 8 张背面（其余由 ×N 表达）
const many = { ...board, seats: board.seats.map((s, i) => (i === 1 ? { ...s, hand_count: 20 } : s)) };
const stackedHtml = renderPublicArea({ ...view, public_board: many }, 0, names);
// 他人座位手牌数：p1=20→8 张背面、p2=3、p3=4、p4=5
const expectedBacks = 8 + 3 + 4 + 5;
assert.equal((stackedHtml.match(/sgs-card back/g) ?? []).length, expectedBacks, "手牌背面最多画 8 张（其余靠 ×N）");
assert.ok(stackedHtml.includes("×20"), "数量标记展示真实张数");

// —— 个人区：手牌正面 / 技能 / 操作条 / pending ——
const meHtml = renderPersonalArea(me, board, {}, names);
assert.ok(meHtml.includes("杀♠7") && meHtml.includes("桃♥3"), "个人区应显示手牌正面");
assert.ok(meHtml.includes("手牌") && meHtml.includes("2/5"), "个人区应显示手牌数/上限");
assert.ok(meHtml.includes('data-skill="rende"'), "技能按钮");
assert.ok(meHtml.includes('data-press="skill"'), "技能按钮参与长按手势");
assert.ok(/data-desc="[^"]+"/.test(meHtml), "技能按钮带简述（长按浮层）");
assert.ok(meHtml.includes("确认打出") && meHtml.includes("disabled"), "未选中时确认按钮置灰");
assert.ok(
  meHtml.indexOf("sgs-me-portrait") < meHtml.indexOf("sgs-me-info") &&
    meHtml.indexOf("sgs-me-info") < meHtml.indexOf("sgs-me-head") &&
    meHtml.indexOf("sgs-me-head") < meHtml.indexOf("sgs-skills") &&
    meHtml.indexOf("sgs-skills") < meHtml.indexOf("sgs-me-row"),
  "个人区：头像左侧，右侧依次为 信息行/技能/装备",
);
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
const discardPendingHtml = renderPersonalArea(withPending, board, {}, names);
assert.ok(
  discardPendingHtml.includes("弃牌：已选") && discardPendingHtml.includes("0</b>/2 张"),
  "弃牌 pending 应显示已选/张数（多选）",
);
assert.ok(
  discardPendingHtml.includes('data-op="discard-confirm"') && discardPendingHtml.includes("确认弃牌"),
  "弃牌 pending 应提供「确认弃牌」一次提交",
);
assert.ok(
  !discardPendingHtml.includes('data-action="discard:'),
  "弃牌 pending 不再逐张按钮提交（改为多选+确认）",
);
assert.ok(
  discardPendingHtml.includes("sgs-card") && discardPendingHtml.includes("discardable"),
  "弃牌 pending 时手牌标记为可多选",
);
// 弃牌数可解释（真机：周泰质疑弃牌数是否被袁术影响）：显示 手牌/上限/原因。
const explainedPending: SgsPlayerArea = {
  ...me,
  pending: {
    kind: "discard",
    actor: "p0",
    options: [{ id: "discard:c1", label: "杀♠7" }],
    data: {
      required: 3,
      reason: "discard_phase",
      reason_label: "手牌超过上限",
      hand: 5,
      limit: 2,
    },
  },
};
const explainedHtml = renderPersonalArea(explainedPending, board, {}, names);
assert.ok(
  explainedHtml.includes("0</b>/3 张") &&
    explainedHtml.includes("手牌 5 / 上限 2") &&
    explainedHtml.includes("手牌超过上限"),
  "弃牌 pending 应显示手牌/上限/原因（不屈升限可解释）",
);
// 庸肆等固定张数弃牌（can_partial 缺省/false）：提示「需一次选满」
const fixedPending: SgsPlayerArea = {
  ...me,
  pending: {
    kind: "discard",
    actor: "p0",
    options: [
      { id: "discard:c1", label: "杀♠7" },
      { id: "discard:c2", label: "桃♥3" },
    ],
    data: { required: 2, reason: "yongsi", reason_label: "庸肆：弃牌阶段先弃 X 张" },
  },
};
const fixedHtml = renderPersonalArea(fixedPending, board, {}, names);
assert.ok(fixedHtml.includes("需一次选满"), "庸肆：固定张数弃牌提示需一次选满");
assert.ok(fixedHtml.includes("庸肆：弃牌阶段先弃 X 张"), "庸肆：显示原因");

// 选中态：外部传入已选牌 → 高亮 + 计数（客户端多选草稿）
const pickedDiscardHtml = renderPersonalArea(withPending, board, {}, names, undefined, undefined, ["c1"]);
assert.ok(
  pickedDiscardHtml.includes("discarded-on") && pickedDiscardHtml.includes("1</b>/2 张"),
  "弃牌多选：已选牌应高亮并计入已选数",
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
const versionedPage = renderSgsPage(gv, {
  gameId: "sanguosha",
  token: "tok-1",
  openid: "p0",
  roomId: "room-9",
  clientVersion: "abc123",
});
assert.ok(
  versionedPage.includes('src="/assets/sgs.js?v=abc123"'),
  "bundle 带内容哈希（避免旧包覆盖 SSR）",
);
assert.ok(pageHtml.includes("<noscript>"), "无 JS 降级");
assert.ok(pageHtml.includes("viewport-fit=cover"), "安全区适配");
assert.ok(!pageHtml.includes("localStorage"), "token 不落 localStorage");
assert.ok(!SGS_STYLES.includes(":has("), "禁用 :has()");
assert.ok(!SGS_STYLES.includes("@container"), "禁用容器查询");
assert.ok(!/\b[0-9]+dvh\b/.test(SGS_STYLES), "禁用 dvh");

// —— 武将卡面：有 map → `<img>`；无 map → 势力色占位（不裂图） ——
const assets = {
  sgs_hero_1: {
    url: "/game-assets/sanguosha/general/aa.webp",
    thumb: "/game-assets/sanguosha/general/aa.t.webp",
  },
  sgs_hero_2: {
    url: "/game-assets/sanguosha/general/bb.webp",
    thumb: "/game-assets/sanguosha/general/bb.t.webp",
  },
};
const avatarHtmlWithMap = renderPublicArea(view, 0, names, assets);
assert.ok(
  avatarHtmlWithMap.includes('class="sgs-avatar sgs-opp-avatar"') &&
    avatarHtmlWithMap.includes("/game-assets/sanguosha/general/"),
  "有素材时应渲染 <img>（thumb）",
);
assert.ok(
  avatarHtmlWithMap.includes("/game-assets/sanguosha/general/aa.t.webp"),
  "座位卡使用缩略图",
);
assert.ok(publicHtml.includes("sgs-avatar sgs-opp-avatar ph"), "无素材时渲染首字占位");
assert.ok(!/<img class="sgs-avatar/.test(publicHtml), "无素材时不得输出图片（避免裂图）");

const pageWithAssets = renderSgsPage(gv, {
  gameId: "sanguosha",
  token: "tok-1",
  openid: "p0",
  roomId: "room-9",
  assets,
});
assert.ok(
  pageWithAssets.includes("/game-assets/sanguosha/general/aa.t.webp"),
  "整页 SSR 带卡面图片",
);
assert.ok(pageWithAssets.includes('"assets"'), "bootstrap 注入 assets（轮询重渲染保留）");
const detailWithAssets = renderGeneralDetail(seats[1], names, assets);
assert.ok(
  detailWithAssets.includes('class="sgs-avatar sgs-detail-avatar"') &&
    detailWithAssets.includes("/game-assets/sanguosha/general/aa.webp"),
  "武将详情使用原图（full）",
);
assert.ok(
  detailWithAssets.includes("sgs-detail-body") &&
    detailWithAssets.indexOf("sgs-detail-portrait") < detailWithAssets.indexOf("sgs-detail\">"),
  "详情浮层图片在右侧（文字环绕）",
);

// —— XSS：昵称必须转义 ——
const chainedSeats = seats.map((s, i) => (i === 1 ? { ...s, chained: true } : s));
const chainedView: SgsViewData = { ...view, public_board: { ...board, seats: chainedSeats } };
assert.ok(renderPublicArea(chainedView, 0, names).includes("横置"), "公共区显示横置标签");
const chainedMe = renderPersonalArea({ ...me, chained: true }, board, {}, names);
assert.ok(chainedMe.includes("横置"), "个人区显示横置标签");
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
const detailWeapon = renderCardDetail({
  ...card("c1", "寒冰剑", "spade", 2, "hanbing_jian"),
  effect: "当你使用【杀】对目标角色造成伤害时，你可以防止此伤害，改为依次弃置其两张牌（不足则全弃）。",
});
assert.ok(
  detailWeapon.includes("效果") && detailWeapon.includes("改为依次弃置其两张牌"),
  "装备牌详情含效果说明（长按武器可看特效）",
);
const generalDetail = renderGeneralDetail(seats[1], names);
assert.ok(generalDetail.includes("曹操") && generalDetail.includes("未公开"), "武将详情（非主公身份不公开）");
const generalDetailSkills = renderGeneralDetail(
  {
    ...seats[1],
    general_name: "界蔡夫人",
    skills: [{ code: "qieting", name: "窃听", type: "normal", implemented: true }],
  },
  names,
);
assert.ok(
  generalDetailSkills.includes("窃听") && generalDetailSkills.includes("其他角色的回合结束后"),
  "武将详情含其他玩家技能与技能描述",
);
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

// —— P4.5：装备技能高亮（equipment_skills + pending.option_sources）——
const hanbingArea = {
  ...me,
  equips: { ...me.equips, weapon: card("c21", "寒冰剑", "spade", 2, "hanbing_jian") },
  equipment_skills: [
    { slot: "weapon", code: "hanbing_jian", name: "寒冰剑", card: "hanbing_jian", trigger: "damage_before" },
  ],
  pending: {
    kind: "choice",
    actor: me.player_id,
    options: [
      { id: "choose:hanbing", label: "寒冰剑：防止此伤害" },
      { id: "pass", label: "正常造成伤害" },
    ],
    data: { option_sources: { "choose:hanbing": "hanbing_jian" } },
  },
} as typeof me;
const hanbingHtml = renderPersonalArea(hanbingArea, board, {}, names);
assert.ok(hanbingHtml.includes('data-equipment-skill="hanbing_jian"'), "装备技能应带 data-equipment-skill");
assert.ok(/sgs-equip active/.test(hanbingHtml), "窗口内对应武器应高亮 active");
const zhangbaArea = {
  ...me,
  equips: { ...me.equips, weapon: card("c22", "丈八蛇矛", "spade", 12, "zhangba_shemao") },
  equipment_skills: [
    { slot: "weapon", code: "zhangba_shemao", name: "丈八蛇矛", card: "zhangba_shemao", trigger: "play_phase" },
  ],
} as typeof me;
const zhangbaHtml = renderPersonalArea(zhangbaArea, board, {}, names, [
  { id: "skill:zhangba:c1,c2", description: "丈八蛇矛：将【A】【B】当【杀】使用" },
]);
assert.ok(/sgs-equip active/.test(zhangbaHtml), "出牌阶段丈八选项应高亮武器");

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
const conversionActions = renderPersonalArea(
  {
    ...me,
    general_name: "界关羽",
    skills: [{ code: "wusheng", name: "武圣", type: "normal", implemented: true }],
  },
  board,
  {},
  names,
  [{ id: "skill:wusheng:c2", description: "武圣：将【桃♥3】当【杀】使用" }],
);
assert.ok(
  /class="sgs-card red playable"[^>]*data-card="c2"/.test(conversionActions),
  "转化技可用的手牌也要标记 playable（武圣主动使用）",
);
assert.ok(
  /data-skill="wusheng"[^>]*data-conversions=/.test(conversionActions) &&
    !/data-skill="wusheng"[^>]*class="sgs-skill disabled"/.test(conversionActions),
  "转化技按钮可点开「用哪张牌」浮层（不置灰）",
);
const zhibaHtml = renderPersonalArea(
  {
    ...me,
    general_name: "界孙策",
    skills: [{ code: "zhiba", name: "制霸", type: "lord", implemented: true }],
  },
  board,
  {},
  names,
  [{ id: "skill:zhiba_jie", description: "制霸：与一名其他吴势力角色拼点" }],
);
assert.ok(
  /data-skill="zhiba"[^>]*data-action="skill:zhiba_jie"/.test(zhibaHtml),
  "界版动作 code 带 _jie 后缀时也应驱动基名技能按钮（界孙策制霸）",
);
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
assert.ok(/class="sgs-skill disabled"/.test(noSkillAction), "无对应动作时技能按钮置灰");
assert.ok(
  !/data-skill="rende"[^>]*data-action=/.test(noSkillAction),
  "无动作时不带 data-action（但保留长按技能简述）",
);

// —— S4：v2 动作 id 兼容（`use:<card>[:targets]` / `convert:<skill>:<card>:<name>`） ——
const v2Actions = [
  { id: "use:c1", description: "使用【杀♠7】" },
  { id: "use:c1:2", description: "使用【杀♠7】→ p2" },
  { id: "use:c2", description: "使用【桃♥3】" },
  { id: "convert:wusheng:c2:杀:2", description: "【桃♥3】当【杀】→ p2" },
  { id: "end_play", description: "结束出牌阶段" },
];
const v2Html = renderPersonalArea(me, board, {}, names, v2Actions);
assert.ok(
  /class="sgs-card[^"]*playable"[^>]*data-card="c1"/.test(v2Html),
  "v2 `use:<card>` 应标记可出",
);
assert.ok(
  /class="sgs-card[^"]*playable"[^>]*data-card="c2"/.test(v2Html),
  "v2 `convert:<skill>:<card>:<name>` 的承载牌应标记可出",
);
assert.ok(!v2Html.includes('data-action="play:c1"'), "v2 不应伪造 v1 play: 动作");
console.log("[sgs-web] v2 动作 id 兼容（use/convert）单测通过 ✅");

console.log("[sgs-web] 出牌链路（可用性/动作按钮）单测通过 ✅");

// —— 视图契约快照（P4.4.1 SP-1）：marks_count 计数标记 + 转换技阳/阴 ——
const luJiArea: SgsPlayerArea = {
  ...me,
  general: "sgs_hero_402",
  general_name: "陆绩",
  marks: {},
  marks_count: { ju: 2 },
  transforms: { shenshi: "yin" },
  skills: [
    { code: "huaiju", name: "怀橘", type: "locked", implemented: true },
    { code: "shenshi", name: "审时", type: "normal", implemented: true },
  ],
};
const luJiPublic = renderPublicArea(
  {
    ...view,
    public_board: {
      ...board,
      seats: board.seats.map((s, i) => (i === 1 ? { ...s, marks: {}, marks_count: { ju: 2 } } : s)),
    },
  },
  0,
  names,
);
assert.ok(luJiPublic.includes("橘×2"), "公共区应下发并展示计数标记「橘×2」");
assert.ok(!luJiPublic.includes("data-marks-count"), "公共区计数标记不引入额外结构（兼容旧快照）");
const luJiPersonal = renderPersonalArea(luJiArea, board, {}, names);
assert.ok(luJiPersonal.includes("橘×2"), "个人区应展示计数标记「橘×2」");
assert.ok(luJiPersonal.includes("审时（阴）"), "个人区应展示转换技当前面（阴）");
const yangArea: SgsPlayerArea = { ...luJiArea, transforms: { shenshi: "yang" } };
assert.ok(renderPersonalArea(yangArea, board, {}, names).includes("审时（阳）"), "转换技阳面展示");
const legacyArea: SgsPlayerArea = { ...luJiArea, marks_count: undefined, transforms: undefined };
const legacyHtml = renderPersonalArea(legacyArea, board, {}, names);
assert.ok(!legacyHtml.includes("橘×"), "旧快照（无 marks_count/transforms）渲染不报错");
// —— SP-2：废除装备栏 / 判定区 / 罪论特殊区 ——
const luKangArea: SgsPlayerArea = {
  ...me,
  general: "sgs_hero_414",
  general_name: "陆抗",
  abolished_slots: ["armor", "horse_offense"],
  judge_abolished: true,
};
const luKangHtml = renderPersonalArea(luKangArea, board, {}, names);
assert.ok(luKangHtml.includes('data-slot="armor"') && luKangHtml.includes("废除"), "个人区：废除装备栏占位");
assert.ok(luKangHtml.includes('data-abolished="1"'), "个人区：废除装备栏带标记");
assert.ok(luKangHtml.includes("判定</span><span class=\"muted\">废除"), "个人区：判定区废除");
const zhuGeArea: SgsPlayerArea = { ...me, general: "sgs_hero_410", general_name: "诸葛瞻" };
const zuiHtml = renderPersonalArea(zhuGeArea, board, {
  zuilun: { cards: [card("c1", "杀", "spade", 7), card("c2", "桃", "heart", 3, "tao")], conditions: 2 },
} as never, names);
assert.ok(zuiHtml.includes('data-zone="zuilun"'), "个人区：罪论特殊区");
assert.ok(zuiHtml.includes("满足 2 项"), "个人区：罪论条件数");
assert.ok(zuiHtml.includes("杀♠7"), "个人区：罪论亮牌");
const abolishedPublic = renderPublicArea(
  {
    ...view,
    public_board: {
      ...board,
      seats: board.seats.map((s, i) =>
        i === 1 ? { ...s, abolished_slots: ["weapon", "armor"], judge_abolished: true } : s,
      ),
    },
  },
  0,
  names,
);
assert.ok(abolishedPublic.includes("判定区已废除"), "公共区：判定区废除提示");
assert.ok(abolishedPublic.includes("废除装备栏×2"), "公共区：废除装备栏数量");
console.log("[sgs-web] 视图契约快照（marks_count / 转换技阳阴）单测通过 ✅");
// SP-2/一将成名：移出游戏（徐盛·破军）牌数展示。
const voidHtml = renderPublicArea(
  { ...view, public_board: { ...board, deck: { ...board.deck, void: 2 } } },
  0,
  names,
);
assert.ok(voidHtml.includes("移出游戏") && voidHtml.includes(">2<"), "公共区：移出游戏牌数");
console.log("[sgs-web] 视图契约快照（废除装备栏/判定区/罪论）单测通过 ✅");

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

// 五谷无懈窗口：候选牌公示（N 张亮出后再决定是否无懈）
const wuguWuxieHtml = renderWuxieOverlay(wuxiePending, board, names, {
  wugu: {
    visibility: "public",
    cards: [card("w1", "杀", "spade", 7), card("w2", "闪", "heart", 2)],
    choosers: ["p0", "p1"],
    cursor: 0,
    picked: [],
  },
});
assert.ok(wuguWuxieHtml.includes("五谷丰登候选牌"), "五谷无懈窗口展示候选牌标题");
assert.ok(wuguWuxieHtml.includes("杀♠7") && wuguWuxieHtml.includes("闪♥2"), "五谷无懈窗口展示候选牌面");
assert.ok(!wuguWuxieHtml.includes('data-action="pick:w1"'), "候选牌只展示，不作为无懈动作");

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
const tiesuoFirst = renderTargetOverlay(
  {
    kind: "target",
    actor: "p0",
    options: [
      { id: "target:p1", label: "选择 李四（已横置）" },
      { id: "recast:c1", label: "重铸：弃置【铁索连环♣10】并摸 1 张（不使用）" },
    ],
    data: { stage: "tiesuo_first", card: "c1" },
  },
  board,
  names,
  () => "铁索连环♣10",
);
assert.ok(tiesuoFirst.includes("第一名目标"), "铁索第一段提示");
assert.ok(tiesuoFirst.includes('data-action="recast:c1"'), "铁索第一段提供重铸入口");
assert.ok(tiesuoFirst.includes("已横置"), "目标选项带横置标签");
const tiesuoSecond = renderTargetOverlay(
  {
    kind: "target",
    actor: "p0",
    options: [{ id: "target:p2", label: "选择 王五（未横置）" }, { id: "confirm", label: "就选 李四" }],
    data: { stage: "tiesuo_second", card: "c1" },
  },
  board,
  names,
);
assert.ok(tiesuoSecond.includes("第二名目标") && !tiesuoSecond.includes("重铸"), "铁索第二段不再给重铸");
assert.ok(tiesuoSecond.includes('data-action="confirm"'), "铁索第二段可就选一名");

// 解围/巧变：移动场牌（move: 选牌 → to: 选目的角色）必须出现在浮层里
// （此前白名单只认 target/targets/recast → 列表渲染为空，真机「解围选择目标为空」）。
const jieweiMove = renderTargetOverlay(
  {
    kind: "target",
    actor: "p0",
    options: [
      { id: "move:c1", label: "移动 曹仁 的装备【青釭剑♠6】" },
      { id: "move:c2", label: "移动 李四 的装备【丈八蛇矛♠Q】" },
    ],
    data: { stage: "jiewei" },
  },
  board,
  names,
);
assert.ok(
  jieweiMove.includes('data-action="move:c1"') && jieweiMove.includes('data-action="move:c2"'),
  "解围：移动来源（含自己）应渲染为可点击选项",
);
assert.ok(
  jieweiMove.includes("随机移动一张手牌"),
  "解围：选牌阶段提示应说明可随机移动手牌",
);
// 解围三阶段：先选来源玩家（msrc:）——同样必须在浮层里可点。
const jieweiSrc = renderTargetOverlay(
  {
    kind: "target",
    actor: "p0",
    options: [
      { id: "msrc:p0", label: "曹仁（手牌 2 / 装备 1 / 判定 0）" },
      { id: "msrc:p1", label: "李四（手牌 3 / 装备 0 / 判定 1）" },
    ],
    data: { stage: "jiewei_src" },
  },
  board,
  names,
);
assert.ok(
  jieweiSrc.includes('data-action="msrc:p0"') && jieweiSrc.includes("来源玩家"),
  "解围：来源玩家（含自己）应渲染为可点击选项",
);
const jieweiDest = renderTargetOverlay(
  {
    kind: "target",
    actor: "p0",
    options: [
      { id: "to:p1", label: "移动到 李四 的装备区" },
      { id: "to:p2", label: "移动到 王五 的判定区" },
    ],
    data: { stage: "jiewei_dest", card: "c1" },
  },
  board,
  names,
);
assert.ok(
  jieweiDest.includes('data-action="to:p1"') && jieweiDest.includes("排除源玩家"),
  "解围：目的角色选项应渲染并提示不能是原角色",
);

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

// —— S4-b：v2 专用浮层（拼点/火攻/借刀/勤王） ——
const pindianHtml = renderPindianOverlay(
  {
    kind: "pindian",
    actor: "p0",
    skill: "pindian",
    options: [
      { id: "choose:c1", label: "用【杀♠7】拼点" },
      { id: "pass", label: "放弃" },
    ],
    data: { target: "p1" },
  },
  board,
  names,
);
assert.ok(pindianHtml.includes("拼点") && pindianHtml.includes('data-action="choose:c1"'), "拼点浮层含选项");
assert.ok(pindianHtml.includes("平局"), "拼点浮层提示平局规则");

const fireHtml = renderFireAttackOverlay(
  {
    kind: "fire_attack",
    actor: "p1",
    skill: "__card",
    options: [{ id: "reveal:c1", label: "展示【杀♠7】" }],
    data: { target: "p1" },
  },
  { ...board, revealed: [{ id: "c9", name: "桃", suit: "heart", rank: 3 } as never] },
  names,
);
assert.ok(fireHtml.includes("火攻") && fireHtml.includes('data-action="reveal:c1"'), "火攻浮层含选项");
assert.ok(fireHtml.includes("已亮出"), "火攻浮层展示亮出的牌");

const collateralHtml = renderCollateralOverlay(
  {
    kind: "collateral",
    actor: "p1",
    skill: "__card",
    options: [
      { id: "respond:c2", label: "打出【杀♦5】" },
      { id: "pass", label: "不打出" },
    ],
    data: {},
  },
  board,
  names,
);
assert.ok(collateralHtml.includes("借刀") && collateralHtml.includes('data-action="respond:c2"'), "借刀浮层含选项");

const qinwangHtml = renderQinwangOverlay(
  {
    kind: "qinwang_sha",
    actor: "p2",
    skill: "qinwang",
    options: [
      { id: "choose:c3", label: "代出【杀♣8】" },
      { id: "pass", label: "不代出" },
    ],
    data: {},
  },
  board,
  names,
);
assert.ok(qinwangHtml.includes("勤王") && qinwangHtml.includes('data-action="choose:c3"'), "勤王浮层含选项");

assert.equal(overlayKindFor({ kind: "pindian" } as never), "pindian");
assert.equal(overlayKindFor({ kind: "fire_attack" } as never), "fire_attack");
assert.equal(overlayKindFor({ kind: "collateral" } as never), "collateral");
assert.equal(overlayKindFor({ kind: "qinwang_sha" } as never), "qinwang_sha");
assert.equal(overlayKindFor({ kind: "harvest" } as never), "wugu", "v2 harvest 复用五谷浮层");
// 蛊惑面板（W3）：扣置牌 + 声明名，仅本人可见
const guhuoHtml = renderPersonalArea(
  {
    ...me,
    pending: {
      kind: "choice",
      actor: me.player_id,
      skill: "guhuo",
      options: [{ id: "declare:闪", label: "蛊惑为【闪】" }],
      data: { stage: "declare", carrier: me.hand?.[0]?.id ?? "c1", declared: "" },
    },
  },
  board,
  {},
  names,
  [],
);
assert.ok(guhuoHtml.includes('data-zone="guhuo"') && guhuoHtml.includes("蛊惑"), "蛊惑面板可见");

// 化身面板（W2）：展示化身牌 + 当前展示标记
const huashenHtml = renderPersonalArea(
  me,
  board,
  {
    huashen: {
      visibility: "owner",
      cards: [
        { code: "sgs_hero_1", name: "刘备", active: true },
        { code: "sgs_hero_2", name: "关羽", active: false },
      ],
      active: "sgs_hero_1",
    } as never,
  },
  names,
  [],
);
assert.ok(
  huashenHtml.includes('data-zone="huashen"') && huashenHtml.includes("展示中"),
  "化身面板展示当前化身",
);

// 观星面板（W6）：展示私密牌 + 当前张数进度
const gxPanelHtml = renderPersonalArea(
  {
    ...me,
    pending: {
      kind: "choice",
      actor: me.player_id,
      skill: "guanxing",
      options: [
        { id: "top", label: "置于牌堆顶" },
        { id: "bottom", label: "置于牌堆底" },
      ],
      data: { cards: ["c1", "c2"], cursor: 1 },
    },
    private_reveals: [
      { id: "c1", name: "杀", suit: "spade", rank: 7 } as never,
      { id: "c2", name: "闪", suit: "heart", rank: 2 } as never,
    ],
  },
  board,
  { guanxing: { visibility: "owner", rows: 2, cols: 5, slots: {}, rev: 0, cards: [] } as never },
  names,
  [],
);
assert.ok(
  gxPanelHtml.includes('data-zone="guanxing"') && gxPanelHtml.includes("2 / 2"),
  "观星面板显示进度",
);

console.log("[sgs-web] v2 专用浮层（拼点/火攻/借刀/勤王）单测通过 ✅");

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

// 草稿来源（P4.6）：新一次观星不得复用上次槽位；槽位含未知 id 时按 cards 重建
const draftA = draftFromZone({
  rev: 1,
  cards: [{ id: "c3" }, { id: "c1" }, { id: "c2" }],
  slots: gx0,
});
assert.deepEqual([draftA.slots.L1, draftA.slots.L2, draftA.slots.L3], ["c3", "c1", "c2"], "草稿沿用合法槽位");
const draftB = draftFromZone({
  rev: 1,
  cards: [{ id: "c7" }, { id: "c8" }, { id: "c9" }],
  slots: gx0, // 上一次观星的槽位（ids 不在本次 cards 中）
});
assert.deepEqual(
  [draftB.slots.L1, draftB.slots.L2, draftB.slots.L3, draftB.slots.R1],
  ["c7", "c8", "c9", null],
  "旧观星槽位必须丢弃并按本次 cards 重建（否则显示裸 id）",
);
assert.notEqual(draftA.sig, draftB.sig, "草稿指纹随牌集变化");

// 提交 id
assert.equal(arrangeAction(tap1), "arrange:c1,c2|c3", "列分组显式提交（左→顶 / 右→底）");
assert.equal(arrangeAction(initialSlots([])), "arrange:|");

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
assert.ok(gxHtml.includes('data-action="arrange:c3,c1,c2|"'), "提交动作 id（右列为空）");
assert.ok(
  gxHtml.includes("置于牌堆顶（顶→下）：杀♠3 → 闪♥A → 桃♦2") &&
    gxHtml.includes("置于牌堆底（靠原牌堆→最底）：无"),
  "顶/底分组预览",
);

// —— P6.3 真机回归：选将阶段必须渲染「候选武将」按钮（此前只有空白面板，表现为卡死） ——
const setupBoard = {
  ...board,
  setup: { lord: "p1", lord_picked: true, picked_count: 4 },
};
const setupArea = {
  ...me,
  general: null,
  general_name: "",
  skills: [],
  hand: [],
  setup: {
    candidates: ["sgs_hero_1", "sgs_hero_2"],
    candidate_details: [
      { code: "sgs_hero_1", name: "刘备", kingdom: "shu", max_hp: 4, is_lord: true, skills: ["仁德", "激将"] },
      { code: "sgs_hero_2", name: "关羽", kingdom: "shu", max_hp: 4, is_lord: false, skills: ["武圣"] },
    ],
    picked: null,
    is_lord: false,
  },
};
const setupHtml = renderPersonalArea(setupArea as never, setupBoard as never, {}, names, [
  { id: "setup:choose_general:sgs_hero_1", description: "选择武将：刘备" },
  { id: "setup:choose_general:sgs_hero_2", description: "选择武将：关羽" },
]);
assert.ok(setupHtml.includes('data-action="setup:choose_general:sgs_hero_1"'), "选将：候选按钮可点击");
assert.ok(setupHtml.includes("刘备") && setupHtml.includes("关羽"), "选将：显示武将名");
assert.ok(setupHtml.includes("仁德") && setupHtml.includes("武圣"), "选将：显示技能名");
assert.ok(setupHtml.includes("选择武将（2 选 1"), "选将：标题带候选数");

const pickedHtml = renderPersonalArea(
  { ...setupArea, setup: { ...setupArea.setup, picked: "sgs_hero_2" } } as never,
  setupBoard as never,
  {},
  names,
  [],
);
assert.ok(pickedHtml.includes("已选武将") && pickedHtml.includes("关羽"), "选将后显示已选武将");
assert.ok(pickedHtml.includes("等待其他玩家选将（4/5）"), "选将后显示进度");
assert.ok(!pickedHtml.includes('data-action="setup:choose_general:'), "选将后不再显示候选");

console.log("[sgs-web] 选将面板单测通过 ✅");

// —— P6.3：选将阶段主公武将应立即公示（其他人的仍隐藏） ——
const setupBoard2 = {
  ...board,
  setup: { lord: "p0", lord_picked: true, picked_count: 2 },
  seats: board.seats.map((s) =>
    s.player_id === "p0"
      ? { ...s, identity: "lord", general_name: "界刘备", general: "sgs_hero_299" }
      : { ...s, identity: null, general_name: "", general: null },
  ),
};
const setupPublic = renderPublicArea(
  { phase: "setup", public_board: setupBoard2, player_area: null, special_zones: {} } as never,
  me.seat,
  names,
);
assert.ok(setupPublic.includes("界刘备"), "选将阶段应公示主公武将");
assert.ok(setupPublic.includes("主公 界刘备"), "公共区回合行显示主公武将");
assert.ok(setupPublic.includes("选将中 2/5"), "选将进度");
console.log("[sgs-web] 主公武将公示单测通过 ✅");

console.log("[sgs-web] 观星纯函数/渲染单测通过 ✅");

// 昵称替换（2026-11-11 真机：局内日志/选项显示 openid）
const nickMap = { BE6B1438FD0256170B06B12F47EF2812: "曹叡" };
assert.equal(
  applyNames("bot:5 对 BE6B1438FD0256170B06B12F47EF2812 造成 1 点伤害（2）", nickMap),
  "机器人5 对 曹叡 造成 1 点伤害（2）",
);
assert.equal(applyNames("bot:4 回复 1 点体力（体力 4，原因：huituo）", undefined), "机器人4 回复 1 点体力（体力 4，原因：huituo）");
assert.equal(applyNames("没有 id", {}), "没有 id");
const nickView = {
  public_board: {
    history: [{ seq: 1, turn: 1, text: "BE6B1438FD0256170B06B12F47EF2812 使用【杀】 → bot:3" }],
    pending: { kind: "target", options: [{ id: "target:4", label: "令 bot:4 判定" }] },
  },
  player_area: { pending: { options: [{ id: "opt1", label: "交给 BE6B1438FD0256170B06B12F47EF2812" }] } },
};
assert.ok(withDisplayNames(nickView, nickMap).public_board.history[0].text.includes("曹叡 使用【杀】 → 机器人3"));
assert.equal(nickView.public_board.pending.options[0].label, "令 机器人4 判定");
assert.equal(nickView.player_area.pending.options[0].label, "交给 曹叡");
assert.equal(nickView.public_board.history[0].text.includes("BE6B"), false, "日志里不应再出现 openid");
console.log("[sgs-web] 昵称替换单测通过 ✅");
