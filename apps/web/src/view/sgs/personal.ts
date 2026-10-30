// 玩家个人资源区（交互基座）：武将/技能/手牌/装备/判定/操作条/pending。
//
// 规格：`docs/games/sanguosha/ui-spec.md` §4。P3.1 先渲染结构（按钮禁用），
// 交互在 P3.2/P3.3 接线（`client/sgs.ts`）。

import { escapeHtml } from "../format.js";
import {
  MARK_LABEL,
  EQUIP_SLOT_LABEL,
  EQUIP_SLOT_ORDER,
  IDENTITY_LABEL,
  KINGDOM_LABEL,
  cardChipHtml,
  cardFaceHtml,
  cardLabel,
  hpPips,
  judgeIcon,
  playerName,
} from "./format.js";
import type { SgsCard, SgsPlayerArea, SgsPublicBoard, SgsSpecialZones } from "./types.js";

function pendingSummary(area: SgsPlayerArea): string {
  const p = area.pending;
  if (!p) return "";
  const extra =
    p.kind === "discard" && typeof p.data?.required === "number"
      ? `（需弃 ${p.data.required} 张，可逐张点）`
      : p.kind === "arrange"
        ? "（观星：P3.5 提供槽位，这里可用默认顺序）"
        : "";
  const buttons = p.options
    .map(
      (o) =>
        `<button type="button" class="btn small${o.id === "pass" ? " ghost" : ""}" ` +
        `data-action="${escapeHtml(o.id)}">${escapeHtml(o.label)}</button>`,
    )
    .join("");
  return (
    `<div class="sgs-pending" data-kind="${escapeHtml(p.kind)}">` +
    `<div class="sgs-pending-head">待办：${escapeHtml(p.kind)}${extra}</div>` +
    `<div class="sgs-pending-actions">${buttons}</div>` +
    `</div>`
  );
}

function wuguPanel(zones: SgsSpecialZones | undefined, names?: Record<string, string>): string {
  const w = zones?.wugu;
  if (!w) return "";
  const cards = (w.cards ?? [])
    .map(
      (c) =>
        `<button type="button" class="sgs-wugu-card" data-pick="${escapeHtml(c.id)}" title="${escapeHtml(
          cardLabel(c),
        )}">${escapeHtml(cardLabel(c))}</button>`,
    )
    .join("");
  const picked = (w.picked ?? [])
    .map((p) => `${escapeHtml(playerName(names, p.player))}：${escapeHtml(cardLabel(p.card))}`)
    .join("；");
  return (
    `<div class="sgs-wugu" data-zone="wugu">` +
    `<div class="sgs-wugu-head">五谷丰登 · 当前 ${escapeHtml(
      w.current ? playerName(names, w.current) : "-",
    )}</div>` +
    `<div class="sgs-wugu-cards">${cards}</div>` +
    (picked ? `<div class="muted">已选：${picked}</div>` : "") +
    `</div>`
  );
}

function marksHtml(area: SgsPlayerArea): string {
  const entries = Object.entries(area.marks ?? {}).filter(([, cards]) => (cards ?? []).length > 0);
  const counts = Object.entries(area.marks_count ?? {}).filter(([, n]) => n > 0);
  if (entries.length === 0 && counts.length === 0) return "";
  const body = [
    ...entries.map(([code, cards]) => {
      const name = MARK_LABEL[code] ?? code;
      return `${escapeHtml(name)}：${(cards ?? []).map((c) => cardChipHtml(c)).join("")}`;
    }),
    ...counts.map(([code, n]) => `${escapeHtml(MARK_LABEL[code] ?? code)}×${n}`),
  ].join(" ");
  return `<div class="sgs-marks" data-marks>${body}</div>`;
}

/** 个人资源区。 */
export interface SgsActionItem {
  id: string;
  description: string;
}

export function renderPersonalArea(
  area: SgsPlayerArea | null | undefined,
  board: SgsPublicBoard,
  zones: SgsSpecialZones | undefined,
  names?: Record<string, string>,
  actions?: SgsActionItem[],
): string {
  if (!area) return `<section class="sgs-personal muted">个人资源区不可见（未坐下或已阵亡）</section>`;
  const acts = actions ?? [];
  const canEndPlay = acts.some((a) => a.id === "end_play");
  const playable = new Set(
    acts.filter((a) => a.id.startsWith("play:")).map((a) => a.id.slice("play:".length)),
  );
  const skillsWithAction = new Map<string, string>();
  for (const a of acts) {
    const m = /^skill:([a-z_]+)(?::(.+))?$/.exec(a.id);
    if (m) skillsWithAction.set(m[2] ?? m[1], a.id);
  }
  const kingdom = area.kingdom ? KINGDOM_LABEL[area.kingdom] ?? area.kingdom : "";
  const general = area.general_name || "未选将";
  const transforms = area.transforms ?? {};
  const skills = (area.skills ?? [])
    .map(
      (s) => {
        const action = skillsWithAction.get(s.code);
        // 转换技（SP-1）：展示当前面（阳/阴）。
        const face = transforms[s.code] === "yin" ? "（阴）" : transforms[s.code] === "yang" ? "（阳）" : "";
        return (
          `<button type="button" class="sgs-skill${s.implemented === false ? " disabled" : ""}" ` +
          `data-skill="${escapeHtml(s.code)}" title="${escapeHtml(s.type)}"` +
          (face ? ` data-face="${face === "（阴）" ? "yin" : "yang"}"` : "") +
          (action ? ` data-action="${escapeHtml(action)}"` : "") +
          `${s.implemented === false || !action ? " disabled" : ""}>` +
          `${escapeHtml(s.name)}${face}</button>`
        );
      },
    )
    .join("");
  const hand = (area.hand ?? [])
    .map((c) => cardFaceHtml(c, playable.has(c.id) ? " playable" : ""))
    .join("");
  // 装备技能高亮（P4.5）：pending.data.option_sources / 出牌阶段丈八选项。
  const optionSources = ((area.pending?.data as Record<string, unknown> | undefined)
    ?.option_sources ?? {}) as Record<string, string>;
  const activeSources = new Set<string>(Object.values(optionSources));
  if (actions?.some((a) => a.id.startsWith("skill:zhangba:"))) {
    activeSources.add("zhangba_shemao");
  }
  const skillBySlot = new Map<string, { code: string; name: string }>();
  for (const s of area.equipment_skills ?? []) {
    skillBySlot.set(s.slot, { code: s.code, name: s.name });
  }
  const equips = EQUIP_SLOT_ORDER.map((slot) => {
    const card = area.equips?.[slot] ?? null;
    const skill = skillBySlot.get(slot);
    const active = skill ? activeSources.has(skill.code) : false;
    const body = card
      ? cardChipHtml(card as SgsCard, { long: true })
      : `<span class="muted">空</span>`;
    return (
      `<div class="sgs-equip${active ? " active" : ""}" data-slot="${slot}"` +
      (skill ? ` data-equipment-skill="${escapeHtml(skill.code)}"` : "") +
      (skill ? ` title="${escapeHtml(skill.name)}"` : "") +
      `><span class="sgs-equip-label">${EQUIP_SLOT_LABEL[slot]}</span>${body}</div>`
    );
  }).join("");
  const judge = (area.judge_zone ?? [])
    .slice()
    .reverse()
    .map((c) => cardChipHtml(c, { icon: judgeIcon(c) }))
    .join("");
  const isMyTurn = board.turn.seat === area.seat && board.turn.phase === "play";
  return (
    `<section class="sgs-personal" aria-label="个人资源区">` +
    `<div class="sgs-me">` +
    `<div class="sgs-me-head"><b>${escapeHtml(general)}</b>` +
    (area.flipped ? `<span class="sgs-flip">翻面</span>` : "") +
    (kingdom ? `<span class="sgs-kingdom">${escapeHtml(kingdom)}</span>` : "") +
    `<span class="sgs-identity ${escapeHtml(area.identity)}">${escapeHtml(
      IDENTITY_LABEL[area.identity] ?? area.identity,
    )}</span>` +
    `<span class="muted">${escapeHtml(playerName(names, area.player_id))}</span></div>` +
    `<div class="sgs-hp">${hpPips(area.hp, area.max_hp)}<span class="sgs-hp-num">${area.hp}/${area.max_hp}</span></div>` +
    `<div class="sgs-skills">${skills}</div>` +
    marksHtml(area) +
    `</div>` +
    `<div class="sgs-me-row">` +
    `<div class="sgs-equips">${equips}</div>` +
    `<div class="sgs-judge"><span class="sgs-equip-label">判定</span>${judge || `<span class="muted">无</span>`}</div>` +
    `</div>` +
    `<div class="sgs-hand" role="list">${hand || `<span class="muted">无手牌</span>`}</div>` +
    pendingSummary(area) +
    wuguPanel(zones, names) +
    `<div class="sgs-ops" data-my-turn="${isMyTurn ? "1" : "0"}">` +
    `<span class="sgs-selected muted">未选择</span>` +
    `<button type="button" class="btn" data-op="confirm" disabled>确认打出</button>` +
    `<button type="button" class="btn ghost" data-op="cancel" disabled>取消选择</button>` +
    `<button type="button" class="btn ghost" data-action="end_play"${canEndPlay ? "" : " disabled"}>结束出牌</button>` +
    `</div>` +
    `</section>`
  );
}
