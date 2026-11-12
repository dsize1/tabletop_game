// 玩家个人资源区（交互基座）：武将/技能/手牌/装备/判定/操作条/pending。
//
// 规格：`docs/games/sanguosha/ui-spec.md` §4。P3.1 先渲染结构（按钮禁用），
// 交互在 P3.2/P3.3 接线（`client/sgs.ts`）。

import { escapeHtml } from "../format.js";
import { renderDropPicker, renderPairPicker } from "./targetPicker.js";
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
  avatarHtml,
  playerName,
} from "./format.js";
import { skillDescription } from "./skill-text.js";
import type {
  SgsAssetMap,
  SgsCard,
  SgsPlayerArea,
  SgsPublicBoard,
  SgsSpecialZones,
} from "./types.js";

/** 待办选项 → 动作列表（targetPicker 的入参形状）。 */
function dropActs(options: Array<{ id: string; label: string }>): Array<{ id: string; description: string }> {
  return options.map((o) => ({ id: o.id, description: o.label }));
}

function pendingSummary(
  area: SgsPlayerArea,
  discardSelected: string[] = [],
  pairPicked: string[] = [],
  dropPicked: number | null = null,
  seats: Array<{ seat: number; label: string }> = [],
): string {
  const p = area.pending;
  if (!p) return "";
  // 弃牌 pending：手牌上多选 + 一次确认（多人反馈：逐张点太慢且易误触）。
  // 确认后若仍超上限，服务端会再次下发 discard pending（直至手牌 ≤ 上限）。
  if (p.kind === "discard" && typeof p.data?.required === "number") {
    const required = Number(p.data.required);
    const partialOk = p.data?.can_partial === true;
    const label = typeof p.data?.reason_label === "string" ? p.data.reason_label : "";
    const hand = typeof p.data?.hand === "number" ? p.data.hand : null;
    const limit = typeof p.data?.limit === "number" ? p.data.limit : null;
    const why = label ? `，${label}` : "";
    const nums = hand !== null && limit !== null ? `：手牌 ${hand} / 上限 ${limit}` : "";
    const selected = discardSelected.filter((id) => (area.hand ?? []).some((c) => c.id === id));
    const canConfirm = partialOk
      ? selected.length >= 1 && selected.length <= required
      : selected.length === required;
    return (
      `<div class="sgs-pending sgs-pending-discard" data-kind="discard" data-required="${required}">` +
      `<div class="sgs-pending-head">弃牌：已选 <b>${selected.length}</b>/${required} 张${nums}${why}` +
      `<span class="muted">（点手牌多选；${
        partialOk ? "可少选，确认后不足会继续提示" : "需一次选满"
      }）</span></div>` +
      `<div class="sgs-pending-actions">` +
      `<button type="button" class="btn" data-op="discard-confirm"${canConfirm ? "" : " disabled"}>确认弃牌</button>` +
      `<button type="button" class="btn ghost" data-op="discard-clear"${selected.length ? "" : " disabled"}>清空选择</button>` +
      `</div></div>`
    );
  }
  const discardDetail =
    p.kind === "discard" && typeof p.data?.required === "number"
      ? (() => {
          const label = typeof p.data?.reason_label === "string" ? p.data.reason_label : "";
          const hand = typeof p.data?.hand === "number" ? p.data.hand : null;
          const limit = typeof p.data?.limit === "number" ? p.data.limit : null;
          const why = label ? `，${label}` : "";
          const nums = hand !== null && limit !== null ? `：手牌 ${hand} / 上限 ${limit}` : "";
          return `（需弃 ${p.data.required} 张${nums}${why}，可逐张点）`;
        })()
      : "";
  const extra =
    discardDetail
      ? discardDetail
      : p.kind === "discard_phase"
        ? "（弃牌阶段：逐张点「弃置」按钮，直到手牌不超过上限）"
        : p.kind === "pindian"
          ? "（拼点：请选择一张手牌亮出，点数大者赢；点数相同时视为平局）"
          : p.kind === "harvest"
            ? "（五谷丰登：从亮出的牌中选择一张）"
            : p.kind === "arrange"
        ? "（观星：P3.5 提供槽位，这里可用默认顺序）"
        : p.skill === "huogong_reveal"
          ? "（火攻：请选择一张手牌亮出）"
          : p.skill === "yijue" && p.data?.stage === "heal"
            ? "（义绝：是否令其回复 1 点体力）"
            : "";
  // 两张牌当群体牌/【杀】：把 C(n,2) 个 `pair:` 按钮折叠成「选两张 → 确认」。
  // 父魂/丈八 → 两张手牌当【杀】；乱击 → 两张同花色手牌当【万箭齐发】。
  const pairActs = p.options.map((o) => ({ id: o.id, description: o.label }));
  const hasPair = p.options.some((o) => o.id.startsWith("pair:"));
  const pairPanel = hasPair
    ? renderPairPicker(
        pairActs,
        (area.hand ?? []).map((c) => ({ id: c.id, label: cardLabel(c) })),
        pairPicked,
        p.skill === "luanji"
          ? { title: "两张同花色手牌当【万箭齐发】", stateText: "可以确认（发动乱击）" }
          : {},
      )
    : "";
  // 乱击第二步「少选一个目标」：drop:<t> 折叠成单选角色 + 确认（「不减少目标」仍走按钮）。
  const dropPanel = p.options.some((o) => o.id.startsWith("drop:"))
    ? renderDropPicker(dropActs(p.options), seats, dropPicked)
    : "";
  const escapeOnly = p.options.filter(
    (o) => !o.id.startsWith("pair:") && !o.id.startsWith("drop:"),
  );
  const buttons = escapeOnly
    .map(
      (o) =>
        `<button type="button" class="btn small${o.id === "pass" ? " ghost" : ""}" ` +
        `data-action="${escapeHtml(o.id)}">${escapeHtml(o.label)}</button>`,
    )
    .join("");
  return (
    `<div class="sgs-pending" data-kind="${escapeHtml(p.kind)}">` +
    `<div class="sgs-pending-head">待办：${escapeHtml(p.kind)}${extra}</div>` +
    `${pairPanel}${dropPanel}` +
    `<div class="sgs-pending-actions">${buttons}</div>` +
    `</div>`
  );
}

/** 观星（v2：逐张顶/底）——展示私密亮出的牌 + 说明；具体选择用待办按钮。 */
function guanxingPanel(area: SgsPlayerArea, zones: SgsSpecialZones | undefined): string {
  const p = area.pending;
  if (!p || p.skill !== "guanxing") return "";
  const cards = zones?.guanxing?.cards ?? area.private_reveals ?? [];
  const rawCursor = p.data?.cursor;
  const cursor = typeof rawCursor === "number" ? (rawCursor as number) : null;
  const total = Array.isArray(p.data?.cards) ? (p.data?.cards as unknown[]).length : cards.length;
  const stepInfo =
    cursor !== null && total > 0
      ? `（第 ${Math.min(cursor + 1, total)} / ${total} 张）`
      : "";
  const current = cursor !== null ? cards[cursor] : undefined;
  const list = cards
    .map(
      (c) =>
        `<span class="sgs-zone-card${current && c.id === current.id ? " on" : ""}">${escapeHtml(
          cardLabel(c),
        )}</span>`,
    )
    .join("");
  return (
    `<div class="sgs-zone-note" data-zone="guanxing">` +
    `<p><b>观星</b>${escapeHtml(stepInfo)}：请逐张选择「置于牌堆顶 / 牌堆底」（可全部置底）。</p>` +
    `<div class="sgs-wuxie-wugu-cards">${list}</div>` +
    `</div>`
  );
}

/** 化身（界左慈，W2）：本人可见的化身牌与当前展示。 */
function huashenPanel(zones: SgsSpecialZones | undefined): string {
  const h = zones?.huashen as
    | {
        cards?: Array<{ code?: string; name?: string; active?: boolean; skills?: string[] }>;
        active?: string | null;
      }
    | undefined;
  if (!h?.cards?.length) return "";
  const chips = h.cards
    .map((c) => {
      const skills = Array.isArray(c.skills) && c.skills.length
        ? `（${(c.skills as string[]).join("、")}）`
        : "";
      return `<span class="sgs-zone-card${c.active ? " on" : ""}">${escapeHtml(
        (c.name ?? c.code ?? "") + (c.active ? "（展示中）" : ""),
      )}${c.active ? escapeHtml(skills) : ""}</span>`;
    })
    .join("");
  return (
    `<div class="sgs-zone-note" data-zone="huashen">` +
    `<p><b>化身</b>：${escapeHtml(String(h.active ?? "未展示"))}</p>` +
    `<div class="sgs-wuxie-wugu-cards">${chips}</div>` +
    `</div>`
  );
}

/** 蛊惑（W3）：扣置一张手牌并声明牌名；翻开前不对其他玩家公开。 */
function guhuoPanel(area: SgsPlayerArea): string {
  const p = area.pending;
  if (!p || p.skill !== "guhuo") return "";
  const declared = typeof p.data?.declared === "string" ? p.data.declared : "";
  const carrier = typeof p.data?.carrier === "string" ? p.data.carrier : "";
  const card = (area.hand ?? []).find((c) => c.id === carrier);
  const stage = typeof p.data?.stage === "string" ? p.data.stage : "";
  const carrierText = card
    ? `已扣置【${cardLabel(card)}】`
    : carrier
      ? `已扣置一张牌（${carrier}）`
      : "";
  return (
    `<div class="sgs-zone-note" data-zone="guhuo">` +
    `<p><b>蛊惑</b>：${escapeHtml(carrierText)}${declared ? `，声明为【${escapeHtml(declared)}】` : ""}` +
    `${stage === "question" ? "；等待质疑" : ""}</p>` +
    `<p class="muted">翻开前其他玩家看不到这张牌；为假时作废并令质疑者摸牌。</p>` +
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
function setupPanel(
  area: SgsPlayerArea,
  board: SgsPublicBoard,
  assets?: SgsAssetMap,
): string {
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
      `<div class="sgs-setup-head">` +
      avatarHtml(picked, name, undefined, assets, { cls: "sgs-setup-avatar", size: [48, 68] }) +
      `<span>已选武将：<b>${escapeHtml(name)}</b></span></div>` +
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
        avatarHtml(d.code, d.name, d.kingdom, assets, { cls: "sgs-candidate-avatar", size: [48, 68] }) +
        `<span class="sgs-candidate-body"><b>${escapeHtml(d.name)}</b>` +
        (meta ? `<span class="muted">${escapeHtml(meta)}</span>` : "") +
        `</span></button>`
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
  assets?: SgsAssetMap,
  discardSelected: string[] = [],
  pairPicked: string[] = [],
  dropPicked: number | null = null,
  seatOptions: Array<{ seat: number; label: string }> = [],
): string {
  if (!area) return `<section class="sgs-personal muted">个人资源区不可见（未坐下或已阵亡）</section>`;
  const acts = actions ?? [];
  const canEndPlay = acts.some((a) => a.id === "end_play");
  // 可点手牌 =
  //  · v1：`play:<card>`
  //  · v2：`use:<card>[:<targets>]`
  //  · 转化技（v1 `skill:<skill>:<card>` / v2 `convert:<skill>:<card>:<牌名>[:<targets>]`）
  const handIds = new Set((area.hand ?? []).map((c) => c.id));
  const playable = new Set(
    acts
      .map((a) => {
        if (a.id.startsWith("play:")) return a.id.slice("play:".length);
        if (a.id.startsWith("use:")) {
          const card = a.id.slice("use:".length).split(":")[0];
          return handIds.has(card) ? card : null;
        }
        const m1 = /^skill:[a-z_]+:(.+)$/.exec(a.id);
        if (m1 && handIds.has(m1[1])) return m1[1];
        const m2 = /^convert:[a-z_]+:([^:]+):/.exec(a.id);
        return m2 && handIds.has(m2[1]) ? m2[1] : null;
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
  const discarding = area.pending?.kind === "discard";
  const discardSet = new Set(discardSelected);
  const hand = (area.hand ?? [])
    .map((c) => {
      const cls = [
        playable.has(c.id) ? "playable" : "",
        discarding ? " discardable" : "",
        discarding && discardSet.has(c.id) ? " discarded-on" : "",
      ]
        .filter(Boolean)
        .join(" ");
      return cardFaceHtml(c, cls ? ` ${cls}` : "");
    })
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
    const press =
      card && !gone
        ? ` data-press="card" data-card="${escapeHtml((card as SgsCard).id)}"`
        : "";
    return (
      `<div class="sgs-equip${active ? " active" : ""}${gone ? " abolished" : ""}" data-slot="${slot}"` +
      (gone ? ' data-abolished="1"' : "") +
      (skill ? ` data-equipment-skill="${escapeHtml(skill.code)}"` : "") +
      (skill ? ` title="${escapeHtml(skill.name)}"` : "") +
      press +
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
    `<div class="sgs-me-portrait">` +
    avatarHtml(area.general, general, area.kingdom, assets, {
      cls: "sgs-me-avatar",
      size: [88, 124],
    }) +
    `</div>` +
    `<div class="sgs-me-info">` +
    `<div class="sgs-me-head">` +
    `<b>${escapeHtml(general)}</b>` +
    (area.flipped ? `<span class="sgs-flip">翻面</span>` : "") +
    (area.chained ? `<span class="sgs-chain">横置</span>` : "") +
    (kingdom ? `<span class="sgs-kingdom">${escapeHtml(kingdom)}</span>` : "") +
    `<span class="sgs-identity ${escapeHtml(area.identity)}">${escapeHtml(
      IDENTITY_LABEL[area.identity] ?? area.identity,
    )}</span>` +
    `<span class="muted">${escapeHtml(playerName(names, area.player_id))}</span>` +
    `<span class="sgs-hp">${hpPips(area.hp, area.max_hp)}<span class="sgs-hp-num">${area.hp}/${area.max_hp}</span></span>` +
    `</div>` +
    `<div class="sgs-skills">${skills}</div>` +
    `<div class="sgs-me-row">` +
    `<div class="sgs-equips">${equips}</div>` +
    `<div class="sgs-judge"><span class="sgs-equip-label">判定</span>${area.judge_abolished ? `<span class="muted">废除</span>` : judge || `<span class="muted">无</span>`}</div>` +
    `</div>` +
    marksHtml(area) +
    `</div>` +
    `</div>` +
    setupPanel(area, board, assets) +
    `<div class="sgs-hand-head"><span class="sgs-equip-label">手牌</span> ${(area.hand ?? []).length}/${handLimitLabel(area.hand_limit)}</div>` +
    `<div class="sgs-hand" role="list">${hand || `<span class="muted">无手牌</span>`}</div>` +
    pendingSummary(area, discardSelected, pairPicked, dropPicked, seatOptions) +
    guanxingPanel(area, zones) +
    guhuoPanel(area) +
    huashenPanel(zones) +
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
