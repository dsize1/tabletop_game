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
  handLimitLabel,
  hpPips,
  judgeIcon,
  playerName,
} from "./format.js";
import { skillDescription } from "./skill-text.js";
import type { SgsCard, SgsPlayerArea, SgsPublicBoard, SgsSpecialZones } from "./types.js";

function pendingSummary(area: SgsPlayerArea): string {
  const p = area.pending;
  if (!p) return "";
  const extra =
    p.kind === "discard" && typeof p.data?.required === "number"
      ? `（需弃 ${p.data.required} 张，可逐张点）`
      : p.kind === "arrange"
        ? "（观星：P3.5 提供槽位，这里可用默认顺序）"
        : p.skill === "huogong_reveal"
          ? "（火攻：请选择一张手牌亮出）"
          : p.skill === "yijue" && p.data?.stage === "heal"
            ? "（义绝：是否令其回复 1 点体力）"
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

/**
 * 选将面板（`phase=setup`）。
 *
 * 历史问题（2026-11-02 真机）：个人区只渲染手牌/技能/结束出牌，**没有候选武将**，
 * 玩家在选将阶段看不到任何可点控件 → 表现为「卡在选将」。
 * 引擎已在 `player_area.setup.candidate_details` 投影名字/势力/体力/技能名。
 */
function setupPanel(area: SgsPlayerArea, board: SgsPublicBoard): string {
  const setup = area.setup;
  if (!setup) return "";
  const details = setup.candidate_details ?? [];
  const picked = setup.picked ?? null;
  const total = board.seat_count || board.seats.length || 0;
  const pickedCount = board.setup?.picked_count ?? 0;
  if (picked) {
    const name = details.find((d) => d.code === picked)?.name ?? area.general_name ?? picked;
    return (
      `<div class="sgs-setup" data-setup="picked">` +
      `<div class="sgs-setup-head">已选武将：<b>${escapeHtml(name)}</b></div>` +
      `<div class="muted">等待其他玩家选将（${pickedCount}/${total}）</div>` +
      `</div>`
    );
  }
  if (details.length === 0) {
    return `<div class="sgs-setup" data-setup="wait"><div class="sgs-setup-head">选将中…</div>` +
      `<div class="muted">等待主公/其他玩家先选（${pickedCount}/${total}）</div></div>`;
  }
  const buttons = details
    .map((d) => {
      const kingdom = d.kingdom ? KINGDOM_LABEL[d.kingdom] ?? d.kingdom : "";
      const hp = typeof d.max_hp === "number" ? `体力 ${d.max_hp}` : "";
      const skills = (d.skills ?? []).join("·");
      const meta = [kingdom, hp, skills].filter(Boolean).join(" · ");
      return (
        `<button type="button" class="sgs-candidate" data-action="setup:choose_general:${escapeHtml(d.code)}">` +
        `<b>${escapeHtml(d.name)}</b>` +
        (meta ? `<span class="muted">${escapeHtml(meta)}</span>` : "") +
        `</button>`
      );
    })
    .join("");
  return (
    `<div class="sgs-setup" data-setup="choose">` +
    `<div class="sgs-setup-head">选择武将（${details.length} 选 1${setup.is_lord ? " · 主公" : ""}）</div>` +
    `<div class="sgs-setup-list">${buttons}</div>` +
    `</div>`
  );
}

/// 罪论（诸葛瞻）亮出的牌：仅本人可见（`visibility: owner`）。
function zuilunPanel(zones: SgsSpecialZones | undefined): string {
  const z = zones?.zuilun as
    | { cards?: { id: string; name?: string; suit?: string; rank?: number }[]; conditions?: number }
    | undefined;
  if (!z || !z.cards || z.cards.length === 0) return "";
  const cards = z.cards
    .map((c) => `<span class="sgs-pick-card">${escapeHtml(cardLabel(c as SgsCard))}</span>`)
    .join("");
  return (
    `<div class="sgs-zuilun" data-zone="zuilun">` +
    `<div class="sgs-wugu-head">罪论 · 牌堆顶 ${z.cards.length} 张（满足 ${z.conditions ?? 0} 项）</div>` +
    `<div class="sgs-wugu-cards">${cards}</div></div>`
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
  // 可点手牌 = 直接 `play:<card>` ∪ 转化技 `skill:<skill>:<card>`
  // （武圣/龙胆/国色/奇袭/连环等：此前只认 play:，导致「武圣不能主动使用」——真机）。
  const handIds = new Set((area.hand ?? []).map((c) => c.id));
  const playable = new Set(
    acts
      .map((a) => {
        if (a.id.startsWith("play:")) return a.id.slice("play:".length);
        const m = /^skill:[a-z_]+:(.+)$/.exec(a.id);
        return m && handIds.has(m[1]) ? m[1] : null;
      })
      .filter((id): id is string => Boolean(id)),
  );
  const skillsWithAction = new Map<string, string>();
  const aliases: Array<{ key: string; id: string }> = [];
  for (const a of acts) {
    const m = /^skill:([a-z_]+)(?::(.+))?$/.exec(a.id);
    if (!m) continue;
    const key = m[2] ?? m[1];
    if (!skillsWithAction.has(key)) skillsWithAction.set(key, a.id);
    // 界版动作 code 可能带 `_jie` 后缀（如 `skill:zhiba_jie`），而武将技能 code 仍是基名
    // （`zhiba`）→ 记录别名，精确动作优先，别名兜底（真机：界孙策制霸按钮置灰）。
    if (!m[2] && m[1].endsWith("_jie")) {
      aliases.push({ key: m[1].slice(0, -"_jie".length), id: a.id });
    }
  }
  for (const a of aliases) {
    if (!skillsWithAction.has(a.key)) skillsWithAction.set(a.key, a.id);
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
        const desc = skillDescription(general, s.name);
        // 转化技（武圣/龙胆/国色/奇袭/连环…）：动作 id 形如 `skill:<code>:<card>`，
        // 按钮本身没有单一动作 → 点开浮层列出「把哪张牌当什么用」。
        const conversions = acts.filter(
          (a) => a.id.startsWith(`skill:${s.code}:`) && !a.id.includes(","),
        );
        // 长按（≥400ms）出技能简述：`data-press` 参与手势委托，`data-desc` 直接带文本。
        // 不可用技能不用 `disabled` 属性（会吞掉 pointer 事件），改用 class + 不给 data-action。
        const usable = s.implemented !== false && (Boolean(action) || conversions.length > 0);
        const convAttr = conversions.length
          ? ` data-conversions="${escapeHtml(
              JSON.stringify(conversions.map((a) => ({ id: a.id, label: a.description }))),
            )}"`
          : "";
        return (
          `<button type="button" class="sgs-skill${usable ? "" : " disabled"}" ` +
          `data-skill="${escapeHtml(s.code)}" data-press="skill" data-skill-name="${escapeHtml(s.name)}" ` +
          `data-desc="${escapeHtml(desc)}" title="${escapeHtml(desc || s.type)}"` +
          (face ? ` data-face="${face === "（阴）" ? "yin" : "yang"}"` : "") +
          (action ? ` data-action="${escapeHtml(action)}"` : "") +
          convAttr +
          `>` +
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
  const abolished = new Set(area.abolished_slots ?? []);
  const equips = EQUIP_SLOT_ORDER.map((slot) => {
    const card = area.equips?.[slot] ?? null;
    const skill = skillBySlot.get(slot);
    const active = skill ? activeSources.has(skill.code) : false;
    const gone = abolished.has(slot);
    const body = gone
      ? `<span class="muted">废除</span>`
      : card
        ? cardChipHtml(card as SgsCard, { long: true })
        : `<span class="muted">空</span>`;
    return (
      `<div class="sgs-equip${active ? " active" : ""}${gone ? " abolished" : ""}" data-slot="${slot}"` +
      (gone ? ' data-abolished="1"' : "") +
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
  // 重铸（铁索连环 / 连环梅花牌）：引擎下发 `recast:<card>`，但卡片点击只会提交
  // `play:<card>` → 必须单独给按钮，否则「没有重铸交互」（2026-11-02 真机）。
  const recasts = acts
    .filter((a) => a.id.startsWith("recast:"))
    .map((a) => {
      const id = a.id.slice("recast:".length);
      const card = (area.hand ?? []).find((c) => c.id === id);
      const label = card ? cardLabel(card) : id;
      return (
        `<button type="button" class="btn ghost sgs-recast" data-action="${escapeHtml(a.id)}">` +
        `重铸【${escapeHtml(label)}】</button>`
      );
    })
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
    setupPanel(area, board) +
    `<div class="sgs-me-row">` +
    `<div class="sgs-equips">${equips}</div>` +
    `<div class="sgs-judge"><span class="sgs-equip-label">判定</span>${area.judge_abolished ? `<span class="muted">废除</span>` : judge || `<span class="muted">无</span>`}</div>` +
    `</div>` +
    `<div class="sgs-hand-head"><span class="sgs-equip-label">手牌</span> ${(area.hand ?? []).length}/${handLimitLabel(area.hand_limit)}</div>` +
    `<div class="sgs-hand" role="list">${hand || `<span class="muted">无手牌</span>`}</div>` +
    pendingSummary(area) +
    wuguPanel(zones, names) +
    zuilunPanel(zones) +
    `<div class="sgs-ops" data-my-turn="${isMyTurn ? "1" : "0"}">` +
    `<span class="sgs-selected muted">未选择</span>` +
    `<button type="button" class="btn" data-op="confirm" disabled>确认打出</button>` +
    `<button type="button" class="btn ghost" data-op="cancel" disabled>取消选择</button>` +
    `<button type="button" class="btn ghost" data-action="end_play"${canEndPlay ? "" : " disabled"}>结束出牌</button>` +
    (recasts ? `<span class="sgs-recasts">${recasts}</span>` : "") +
    `</div>` +
    `</section>`
  );
}
