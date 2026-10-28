// 特殊区 pending 浮层（纯函数，P3.4）：无懈链 / 濒死 / 五谷 / pick_card / 目标 / 技能多选。
//
// 交互语义见 `docs/games/sanguosha/ui-spec.md` §9：无懈链必须显式选择（不可误关）；
// 所有按钮都是可直接提交的 action id（与服务端 `legal_actions` 一致）。

import { escapeHtml } from "../format.js";
import { IDENTITY_LABEL, cardLabel, playerName } from "./format.js";
import type { SgsCard, SgsPending, SgsPlayerArea, SgsPublicBoard, SgsSpecialZones } from "./types.js";

function seatLabel(board: SgsPublicBoard, pid: string | null | undefined, names?: Record<string, string>): string {
  if (!pid) return "-";
  const seat = board.seats.find((s) => s.player_id === pid);
  return seat?.general_name ? `${playerName(names, pid)}（${seat.general_name}）` : playerName(names, pid);
}

function optionButtons(p: SgsPending, filter?: (id: string) => boolean): string {
  return p.options
    .filter((o) => (filter ? filter(o.id) : true))
    .map(
      (o) =>
        `<button type="button" class="btn small${o.id === "pass" ? " ghost" : ""}" ` +
        `data-action="${escapeHtml(o.id)}">${escapeHtml(o.label)}</button>`,
    )
    .join("");
}

/** 无懈链：root + 层数 + 当前询问者 + 打出/不出。 */
export function renderWuxieOverlay(
  p: SgsPending,
  board: SgsPublicBoard,
  names?: Record<string, string>,
): string {
  const root = typeof p.data?.root === "string" ? p.data.root : "";
  const layers = Number(p.data?.layers ?? p.data?.count ?? 0);
  return (
    `<div class="sgs-zone-note">` +
    `<p><b>被抵消的效果</b>：${escapeHtml(root)}</p>` +
    `<p class="muted">已打出无懈：${layers} 层（奇数=原效果被抵消）。当前询问：` +
    `${escapeHtml(seatLabel(board, p.actor, names))}</p>` +
    `<p class="muted">必须显式选择「打出」或「不出」。</p>` +
    `</div>` +
    `<div class="sgs-zone-actions">${optionButtons(p)}</div>`
  );
}

/** 濒死求桃。 */
export function renderDyingOverlay(
  p: SgsPending,
  board: SgsPublicBoard,
  names?: Record<string, string>,
): string {
  const victim = typeof p.data?.victim === "string" ? p.data.victim : null;
  const hp = p.data?.hp;
  return (
    `<p>${escapeHtml(seatLabel(board, victim, names))} 进入濒死` +
    `${typeof hp === "number" ? `（体力 ${hp}）` : ""}，是否使用【桃】救援？</p>` +
    `<div class="sgs-zone-actions">${optionButtons(p)}</div>`
  );
}

/** 五谷丰登：亮出的牌 + 已选结果（公开）。 */
export function renderWuguOverlay(
  p: SgsPending,
  board: SgsPublicBoard,
  zones: SgsSpecialZones | undefined,
  names?: Record<string, string>,
): string {
  const zone = zones?.wugu;
  const cards = zone?.cards ?? [];
  const picked = zone?.picked ?? [];
  const cardsHtml = cards
    .map(
      (c: SgsCard) =>
        `<button type="button" class="sgs-wugu-card" data-action="pick:${escapeHtml(c.id)}">` +
        `${escapeHtml(cardLabel(c))}</button>`,
    )
    .join("");
  const pickedHtml = picked
    .map((x) => `<li>${escapeHtml(playerName(names, x.player))}：${escapeHtml(cardLabel(x.card))}</li>`)
    .join("");
  return (
    `<p class="muted">当前选择：${escapeHtml(
      seatLabel(board, p.data?.chooser as string | undefined, names),
    )}</p>` +
    `<div class="sgs-wugu-cards">${cardsHtml || `<span class="muted">没有可选牌</span>`}</div>` +
    (pickedHtml ? `<p class="muted">已选：</p><ul>${pickedHtml}</ul>` : "") +
    `<div class="sgs-zone-actions">${optionButtons(p)}</div>`
  );
}

/** 拿牌选择（顺手/过河）：装备区 / 手牌（随机）/ 判定区 三组。 */
export function renderPickCardOverlay(
  p: SgsPending,
  board: SgsPublicBoard,
  names?: Record<string, string>,
): string {
  const groups: Array<{ key: string; title: string; filter: (id: string) => boolean }> = [
    { key: "equip", title: "装备区", filter: (id) => id.startsWith("pick:equip:") },
    { key: "hand", title: "手牌（随机 1 张）", filter: (id) => id === "pick:hand" },
    { key: "judge", title: "判定区", filter: (id) => id.startsWith("pick:judge:") },
  ];
  const cardName = typeof p.data?.card === "string" ? p.data.card : "";
  const target = typeof p.data?.target === "string" ? p.data.target : null;
  const sections = groups
    .map((g) => {
      const btns = p.options
        .filter((o) => g.filter(o.id))
        .map(
          (o) =>
            `<button type="button" class="btn small" data-action="${escapeHtml(o.id)}">` +
            `${escapeHtml(o.label)}</button>`,
        )
        .join("");
      return btns ? `<div class="sgs-zone-group"><h3>${g.title}</h3><div class="sgs-zone-actions">${btns}</div></div>` : "";
    })
    .join("");
  return (
    `<p class="muted">目标：${escapeHtml(seatLabel(board, target, names))}` +
    `${cardName ? ` · 牌：${escapeHtml(cardName)}` : ""}</p>` +
    (sections || `<p class="muted">目标没有可选牌，将直接结算。</p>`)
  );
}

/** 目标选择（杀 / 锦囊 / 借刀二阶段）。 */
export function renderTargetOverlay(
  p: SgsPending,
  board: SgsPublicBoard,
  names?: Record<string, string>,
  resolveCardName?: (id: string) => string | null,
): string {
  const stage = typeof p.data?.stage === "string" ? p.data.stage : "single";
  const cardId = typeof p.data?.card === "string" ? p.data.card : "";
  const cardName = cardId && resolveCardName ? resolveCardName(cardId) : cardId;
  const hint =
    stage === "borrow_first"
      ? "选择一名装备区有武器的角色（借谁的刀）"
      : stage === "borrow_second"
        ? "选择该角色攻击范围内的杀目标"
        : stage === "confirm"
          ? "确认目标后开始结算"
          : cardName
            ? `使用【${cardName}】：请选择目标`
            : "请选择目标";
  return (
    `<p class="muted">${escapeHtml(hint)}</p>` +
    `<div class="sgs-zone-actions">` +
    optionButtons(p, (id) => id.startsWith("target:") || id === "confirm" || id === "cancel") +
    `</div>`
  );
}

/** 技能多选（制衡/仁德/离间）：手牌勾选 + 确认/取消（选择状态由引擎回传）。 */
export function renderChooseManyOverlay(
  p: SgsPending,
  area: SgsPlayerArea | null | undefined,
  names?: Record<string, string>,
): string {
  const selected = new Set(
    Array.isArray(p.data?.selected) ? (p.data?.selected as string[]) : [],
  );
  const min = Number(p.data?.min ?? 0);
  const max = Number(p.data?.max ?? 0);
  const purpose = typeof p.data?.purpose === "string" ? p.data.purpose : p.skill ?? "";
  const cards = (area?.hand ?? [])
    .map((c) => {
      const on = selected.has(c.id);
      return (
        `<button type="button" class="sgs-multi-card${on ? " on" : ""}" ` +
        `data-action="choose:${escapeHtml(c.id)}">${escapeHtml(cardLabel(c))}</button>`
      );
    })
    .join("");
  return (
    `<p class="muted">${escapeHtml(purpose)}：已选 ${selected.size} 张（${min}–${max}）` +
    `${names ? "" : ""}</p>` +
    `<div class="sgs-multi-cards">${cards || `<span class="muted">没有手牌</span>`}</div>` +
    `<div class="sgs-zone-actions">${optionButtons(p, (id) => !id.startsWith("choose:"))}</div>`
  );
}

/** 该 pending 是否用浮层呈现（其余留在个人区操作条）。 */
export function overlayKindFor(p: SgsPending): "wuxie" | "dying" | "wugu" | "pick_card" | "target" | "choose_many" | null {
  switch (p.kind) {
    case "wuxie":
    case "dying":
    case "wugu":
    case "pick_card":
    case "target":
    case "choose_many":
      return p.kind;
    default:
      return null;
  }
}

/** 不可误关（必须显式选择）的 pending。 */
export function isBlockingPending(kind: string): boolean {
  return kind === "wuxie" || kind === "dying" || kind === "wugu" || kind === "pick_card";
}

/** 标题。 */
export function pendingTitle(kind: string): string {
  const map: Record<string, string> = {
    wuxie: "无懈可击",
    dying: "濒死求桃",
    wugu: "五谷丰登",
    pick_card: "选择一张牌",
    target: "选择目标",
    choose_many: "选择手牌",
  };
  return map[kind] ?? "待办";
}

/** 身份文案（供未来扩展；保持与公共区一致）。 */
export function identityLabel(id: string | null | undefined): string {
  return id ? IDENTITY_LABEL[id] ?? id : "未公开";
}
