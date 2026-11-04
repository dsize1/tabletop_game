// 公共局面区（只读）：座位环绕 + 中央牌堆/弃牌/阶段 + 公共特殊区。
//
// 规格：`docs/games/sanguosha/ui-spec.md` §3.2 / §4。禁止在公共区放任何可提交控件。

import { escapeHtml } from "../format.js";
import {
  MARK_LABEL,
  IDENTITY_LABEL,
  KINGDOM_LABEL,
  PHASE_LABEL,
  avatarHtml,
  cardBackHtml,
  cardChipHtml,
  cardLabel,
  handLimitLabel,
  hpPips,
  judgeIcon,
  playerName,
  seatPlacements,
  othersInSeatOrder,
} from "./format.js";
import type { SgsAssetMap, SgsCard, SgsPublicBoard, SgsSpecialZones, SgsViewData } from "./types.js";

function seatCard(
  board: SgsPublicBoard,
  seatIdx: number,
  placement: { left: number; top: number; side: "left" | "top" | "right" },
  names?: Record<string, string>,
  assets?: SgsAssetMap,
): string {
  const seat = board.seats[seatIdx];
  if (!seat) return "";
  const cls = [
    "sgs-opponent",
    `anchor-${{ left: "right", right: "left", top: "center" }[placement.side]}`,
    seat.alive ? "" : "dead",
    board.turn.seat === seatIdx && board.turn.phase !== "finished" ? "current" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const identity = seat.identity
    ? `<span class="sgs-identity ${seat.identity}">${IDENTITY_LABEL[seat.identity] ?? seat.identity}</span>`
    : "";
  const kingdom = seat.kingdom ? KINGDOM_LABEL[seat.kingdom] ?? seat.kingdom : "";
  const name = escapeHtml(playerName(names, seat.player_id));
  const flipped = seat.flipped ? `<span class="sgs-flip">翻面</span>` : "";
  // 连环状态（P4.6）：横置是公共信息，必须在公共区可见（属性伤害会传导）。
  const chained = seat.chained ? `<span class="sgs-chain">横置</span>` : "";
  const general = seat.general_name
    ? `<div class="sgs-general">${escapeHtml(seat.general_name)}${flipped}${chained}${kingdom ? `<span class="sgs-kingdom">${escapeHtml(kingdom)}</span>` : ""}</div>`
    : `<div class="sgs-general muted">未选将</div>`;
  const marks = [
    ...Object.entries(seat.marks ?? {})
      .filter(([, cards]) => (cards ?? []).length > 0)
      .map(([code, cards]) => `${MARK_LABEL[code] ?? code}×${cards.length}`),
    ...Object.entries(seat.marks_count ?? {})
      .filter(([, n]) => n > 0)
      .map(([code, n]) => `${MARK_LABEL[code] ?? code}×${n}`),
  ].join(" ");
  const equips = Object.entries(seat.equips ?? {})
    .filter(([, c]) => Boolean(c))
    .map(([slot, c]) => cardChipHtml(c as SgsCard, { long: true }))
    .join("");
  const judge = (seat.judge_zone ?? [])
    .slice()
    .reverse() // 后进在左（栈顶 = 先判）
    .map((c) => cardChipHtml(c, { icon: judgeIcon(c) }))
    .join("");
  // 手牌背面堆叠：每张只露一小部分（CSS 负 margin），最多画 8 张，其余靠 ×N 表达。
  const shownBacks = Math.min(seat.hand_count, 8);
  const handDots = Array.from({ length: shownBacks }, () => cardBackHtml()).join("");
  return (
    `<div class="${cls}" data-press="general" data-seat="${seatIdx}" style="left:${placement.left}%;top:${placement.top}%">` +
    `<div class="sgs-opponent-head">` +
    avatarHtml(seat.general, seat.general_name, seat.kingdom, assets, {
      cls: "sgs-opp-avatar",
      size: [26, 26],
    }) +
    name +
    identity +
    `</div>` +
    general +
    `<div class="sgs-hp">${hpPips(seat.hp, seat.max_hp)}<span class="sgs-hp-num">${seat.hp}/${seat.max_hp}</span></div>` +
    `<div class="sgs-hand-mini" title="手牌数 / 手牌上限">${handDots}<span class="sgs-hand-count">×${seat.hand_count}/${handLimitLabel(seat.hand_limit)}</span></div>` +
    (marks ? `<div class="sgs-mini-row marks">${escapeHtml(marks)}</div>` : "") +
    (equips ? `<div class="sgs-mini-row">${equips}</div>` : "") +
    (judge ? `<div class="sgs-mini-row judge">${judge}</div>` : "") +
    (seat.judge_abolished ? `<div class="sgs-mini-row judge muted">判定区已废除</div>` : "") +
    ((seat.abolished_slots ?? []).length > 0
      ? `<div class="sgs-mini-row muted">废除装备栏×${(seat.abolished_slots ?? []).length}</div>`
      : "") +
    (seat.alive ? "" : `<div class="sgs-dead-mark">阵亡</div>`) +
    `</div>`
  );
}

function publicSpecialZonesHtml(zones: SgsSpecialZones | undefined): string {
  if (!zones) return "";
  const parts: string[] = [];
  const judge = zones.judge as
    | { owner?: string; delayed?: SgsCard; judge_card?: SgsCard; effect?: string }
    | undefined;
  if (judge && judge.judge_card) {
    parts.push(
      `<div class="sgs-zone-item"><b>判定</b> ${judge.delayed ? escapeHtml(cardLabel(judge.delayed)) : ""} → ` +
        `${escapeHtml(cardLabel(judge.judge_card))}${judge.effect ? `（${escapeHtml(judge.effect)}）` : ""}</div>`,
    );
  }
  const wugu = zones.wugu as { cards?: SgsCard[]; current?: string | null } | undefined;
  if (wugu && Array.isArray(wugu.cards)) {
    parts.push(
      `<div class="sgs-zone-item"><b>五谷丰登</b> 剩余 ${wugu.cards.length} 张：` +
        wugu.cards.map((c) => escapeHtml(cardLabel(c))).join(" / ") +
        `</div>`,
    );
  }
  return parts.length ? `<div class="sgs-zone-list">${parts.join("")}</div>` : "";
}

/** 公共局面区。`mySeat` 为本人座位（决定座位环绕顺序）。 */
export function renderPublicArea(
  data: SgsViewData,
  mySeat: number,
  names?: Record<string, string>,
  assets?: SgsAssetMap,
): string {
  const board = data.public_board;
  const n = board.seat_count || board.seats.length;
  const others = othersInSeatOrder(n, mySeat);
  const placements = seatPlacements(others.length);
  const seatsHtml = placements
    .map((p) => seatCard(board, others[p.index], p, names, assets))
    .join("");

  const phase = PHASE_LABEL[board.turn.phase] ?? board.turn.phase;
  const lordSeat = board.seats.find((s) => s.identity === "lord");
  const lordGeneral = lordSeat?.general_name ?? "";
  const setupProgress =
    data.phase === "setup"
      ? ` · 选将中 ${board.setup?.picked_count ?? 0}/${board.seat_count || board.seats.length}` +
        (lordGeneral ? ` · 主公 ${lordGeneral}` : "")
      : "";
  const currentName = board.turn.player_id
    ? escapeHtml(playerName(names, board.turn.player_id))
    : "-";
  const pending =
    board.pending && board.pending.kind !== "wugu"
      ? `<div class="sgs-waiting">等待 ${escapeHtml(playerName(names, board.pending.actor))} 响应（${escapeHtml(board.pending.kind)}）</div>`
      : "";
  // P6.3：机器人回合由 server 自动推进（人类动作后同一次请求内跑完）。
  // 页面/轮询都要给出「机器人行动中」的等待态，避免玩家以为是卡住。
  const actorPid = board.pending?.actor ?? board.turn.player_id;
  const actorSeat = board.seats.find((s) => s.player_id === actorPid);
  const botActing =
    !board.winner && !board.aborted && Boolean(actorPid) && actorSeat?.is_bot === true;
  const inPlay = data.phase === "turn" || data.phase === "dying";
  const botWaiting = botActing && inPlay
    ? `<div class="sgs-waiting sgs-waiting-bot">🤖 ${escapeHtml(
        playerName(names, actorPid as string),
      )} 行动中…（服务器自动推进，无需操作）</div>`
    : "";
  const winner = board.winner
    ? `<div class="sgs-winner">${escapeHtml(board.winner.team)} 阵营胜利</div>`
    : board.aborted
      ? `<div class="sgs-winner">${escapeHtml(board.aborted)}</div>`
      : "";
  const discardTop = board.discard_top
    ? `<div class="sgs-deck-discard">${escapeHtml(cardLabel(board.discard_top))}</div>`
    : `<div class="sgs-deck-discard muted">空</div>`;

  return (
    `<section class="sgs-public" aria-label="公共局面区">` +
    `<div class="sgs-seats">${seatsHtml}</div>` +
    `<div class="sgs-center">` +
    `<div class="sgs-round">第 ${board.round} 轮 · ${escapeHtml(phase)} · ${currentName}${setupProgress}</div>` +
    `<div class="sgs-deck-row">` +
    `<div class="sgs-deck"><span class="sgs-deck-label">牌堆</span>${board.deck.draw}</div>` +
    `<button type="button" class="sgs-deck sgs-deck-btn" data-press="discard" aria-label="查看弃牌堆">` +
    `<span class="sgs-deck-label">弃牌</span>${board.deck.discard}${discardTop}</button>` +
    (board.deck.void > 0
      ? `<div class="sgs-deck" data-zone="void"><span class="sgs-deck-label">移出游戏</span>${board.deck.void}</div>`
      : "") +
    `</div>` +
    botWaiting +
    pending +
    winner +
    publicSpecialZonesHtml(data.special_zones) +
    `</div>` +
    `</section>`
  );
}

/** 左侧辅助区正文：阶段条 + 本局信息 + 出牌历史（客户端每次轮询只刷这部分）。 */
export function renderSidePanelBody(data: SgsViewData, names?: Record<string, string>): string {
  const board = data.public_board;
  // 全量历史（引擎上限 800），最新在最上面；端侧只负责展示顺序。
  const history = (board.history ?? []).slice().reverse();
  const rows = history
    .map((h) => {
      // 历史正文多数以「使用/装备/打出」开头，补上操作者更易复盘。
      const who = h.actor
        ? `<b class="sgs-log-actor">${escapeHtml(playerName(names, h.actor))}</b> `
        : "";
      return `<li data-seq="${h.seq}"><span class="muted">#${h.seq}</span> ${who}${escapeHtml(h.text)}</li>`;
    })
    .join("");
  const phases = ["prepare", "judge", "draw", "play", "discard", "end"]
    .map(
      (p) =>
        `<span class="sgs-phase${board.turn.phase === p ? " on" : ""}">${PHASE_LABEL[p] ?? p}</span>`,
    )
    .join("");
  return (
    `<div class="sgs-side-block"><h3>阶段</h3><div class="sgs-phases">${phases}</div></div>` +
    `<div class="sgs-side-block"><h3>本局</h3>` +
    `<div class="muted">第 ${board.round} 轮 · 第 ${board.turn_index + 1} 回合 · 当前 ${escapeHtml(
      board.turn.player_id ? playerName(names, board.turn.player_id) : "-",
    )}</div></div>` +
    `<div class="sgs-side-block grow"><h3>出牌历史（${history.length}）</h3>` +
    `<ul class="sgs-history" data-count="${history.length}">${rows}</ul></div>`
  );
}

/** 左侧辅助区（外壳，SSR 用；`id="sgs-side"` 供客户端局部刷新）。 */
export function renderSidePanel(data: SgsViewData, names?: Record<string, string>): string {
  return (
    `<aside class="sgs-side" id="sgs-side" aria-label="辅助区">` +
    renderSidePanelBody(data, names) +
    `</aside>`
  );
}
