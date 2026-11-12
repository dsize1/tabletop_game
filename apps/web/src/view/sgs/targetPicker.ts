// 端侧「多段交互」目标选择（2026-11-12 用户裁定：仅 B —— 合法动作集保持平铺，便于 AI/回放；
// 只在端侧把「选牌 → 多选目标 → 确认」折叠成多段交互）。
//
// 合法性由**引擎下发的动作 id**决定：端侧不做规则判断，只用「当前选中的目标集合是否等于某个
// 合法动作的目标集合」来启用确认。于是：
// - 必须 N 名目标的牌（如借刀杀人 = 2）：min === max === 2 → 不足/超出都不合法；
// - 允许 ≤N 的牌（如铁索连环 1–2、方天画戟 1–3）：min–max 之间且组合本身在合法集里才行；
// - 有序语义（借刀杀人：第一个必须是持武器者）：同一集合只有一种合法顺序 → 集合匹配即唯一命中。
import { escapeHtml } from "../format.js";

export interface ActionLike {
  id: string;
  description: string;
}

export interface UseOption {
  /** 可直接提交的 action id（含目标，顺序由引擎给定）。 */
  id: string;
  /** 该动作的目标座位（无目标为空数组）。 */
  targets: number[];
}

export interface UseGroup {
  /** 组键（不含目标）：`use:<card>` / `convert:<skill>:<from>:<to>`。 */
  key: string;
  /** 用法名（已去掉「 → 目标」后缀）。 */
  label: string;
  options: UseOption[];
  /** 合法目标数下界 / 上界。 */
  min: number;
  max: number;
}

const TARGET_SUFFIX = /\s*→\s*[^→]*$/;

/** `1,3` → [1,3]；空 → []。 */
export function parseTargets(raw: string | undefined): number[] {
  if (!raw) return [];
  return raw
    .split(",")
    .map((s) => Number(s.trim()))
    .filter((n) => Number.isInteger(n) && n >= 0);
}

function push(
  groups: Map<string, UseGroup>,
  key: string,
  label: string,
  id: string,
  targets: number[],
) {
  const g =
    groups.get(key) ??
    {
      key,
      label: label.replace(TARGET_SUFFIX, "").trim(),
      options: [],
      min: Number.POSITIVE_INFINITY,
      max: 0,
    };
  if (!g.options.some((o) => o.id === id)) g.options.push({ id, targets });
  g.min = Math.min(g.min, targets.length);
  g.max = Math.max(g.max, targets.length);
  groups.set(key, g);
}

/**
 * 把某张手牌的合法动作按「用法」分组（v2 动作 id 形态）：
 * - `use:<card>` / `use:<card>:<t1,t2>`（普通使用 + 目标）
 * - `convert:<skill>:<from>:<to>` / `…:<t1,t2>`（转化技 + 目标）
 */
export function groupUseActions(cardId: string, actions: ActionLike[]): UseGroup[] {
  const groups = new Map<string, UseGroup>();
  for (const a of actions) {
    const use = /^use:([^:]+)(?::([0-9,]+))?$/.exec(a.id);
    if (use && use[1] === cardId) {
      push(groups, `use:${cardId}`, a.description, a.id, parseTargets(use[2]));
      continue;
    }
    const conv = /^convert:([a-z0-9_]+):([^:]+):([^:]+?)(?::([0-9,]+))?$/.exec(a.id);
    if (conv && conv[2] === cardId) {
      push(
        groups,
        `convert:${conv[1]}:${conv[2]}:${conv[3]}`,
        a.description,
        a.id,
        parseTargets(conv[4]),
      );
    }
  }
  return [...groups.values()];
}

/** 目标数提示文案。 */
export function targetHint(group: UseGroup): string {
  if (group.max === 0) return "无需选择目标";
  if (group.min === group.max) return `必须选择 ${group.min} 名目标`;
  return `可选择 ${group.min}–${group.max} 名目标`;
}

/**
 * 当前选中的目标集合能对应哪个合法动作 id（集合语义；顺序无关）。
 *
 * 返回 null = 该组合不合法（端侧据此禁用「确认」）。借刀杀人这类有序语义依旧唯一命中，
 * 因为反向集合不在合法集里。
 */
export function matchTargets(group: UseGroup, picked: number[]): string | null {
  const key = [...picked].sort((a, b) => a - b).join(",");
  for (const o of group.options) {
    const candidate = [...o.targets].sort((a, b) => a - b).join(",");
    if (candidate === key) return o.id;
  }
  return null;
}

/** 单目标用法：点目标即提交（不强制再点一次确认，保持出杀速度）。 */
export function isSingleTarget(group: UseGroup): boolean {
  return group.max === 1;
}

/** 目标候选座位（组内出现过的目标并集）。 */
export function candidateTargets(group: UseGroup): number[] {
  const set = new Set<number>();
  for (const o of group.options) for (const t of o.targets) set.add(t);
  return [...set].sort((a, b) => a - b);
}

export interface PickerSeat {
  seat: number;
  label: string;
  alive?: boolean | null;
}

/** 渲染目标选择浮层（按钮 data 属性由客户端事件处理）。 */
export function renderTargetPicker(
  group: UseGroup,
  seats: PickerSeat[],
  picked: number[],
  opts: { title?: string } = {},
): string {
  const cands = new Set(candidateTargets(group));
  const rows = seats
    .filter((s) => cands.has(s.seat))
    .map((s) => {
      const on = picked.includes(s.seat);
      return (
        `<button type="button" class="btn small sgs-pick-target${on ? " selected" : ""}" ` +
        `data-pick-target="${s.seat}">${escapeHtml(s.label)}</button>`
      );
    })
    .join("");
  const matched = matchTargets(group, picked);
  const hint = targetHint(group);
  const state = matched
    ? `已选 ${picked.length} 名：可以确认`
    : picked.length === 0
      ? "请选择目标"
      : `已选 ${picked.length} 名：该组合不合法`;
  return (
    `<p class="muted">${escapeHtml(opts.title ?? group.label)} · ${escapeHtml(hint)}</p>` +
    `<div class="sgs-zone-actions sgs-pick-targets">${rows || '<span class="muted">没有可指定的目标</span>'}</div>` +
    `<p class="muted" data-pick-state="1">${escapeHtml(state)}</p>` +
    `<div class="sgs-zone-actions">` +
    `<button type="button" class="btn small" data-pick-confirm="1"${matched ? "" : " disabled"}>确认</button>` +
    `<button type="button" class="btn small" data-pick-cancel="1">取消</button>` +
    `</div>`
  );
}

/** 多种用法（普通使用 / 转化技）→ 先选用法，再选目标。 */
export function renderUsagePicker(groups: UseGroup[]): string {
  const rows = groups
    .map(
      (g) =>
        `<button type="button" class="btn small" data-pick-group="${escapeHtml(g.key)}">` +
        `${escapeHtml(g.label)}` +
        `${g.max > 0 ? `<span class="muted">（${escapeHtml(targetHint(g))}）</span>` : ""}</button>`,
    )
    .join("");
  return `<p class="muted">这张牌有多种用法，先选一种：</p><div class="sgs-zone-actions">${rows}</div>`;
}

// ---------------------------------------------------------------------------
// 两张手牌当【杀】（父魂 / 丈八蛇矛）：响应窗口里的 `pair:c1,c2` 折叠成「选两张 → 确认」
// ---------------------------------------------------------------------------

/** 从响应窗口的选项里取出 `pair:` 组合（两张手牌当【杀】）。 */
export function pairOptions(options: ActionLike[]): Array<{ id: string; cards: [string, string] }> {
  const out: Array<{ id: string; cards: [string, string] }> = [];
  for (const o of options) {
    const m = /^pair:([^,]+),([^,]+)$/.exec(o.id);
    if (m) out.push({ id: o.id, cards: [m[1], m[2]] });
  }
  return out;
}

/** 选中的两张牌能否对应某个合法 `pair:` 动作（集合语义；同一对牌顺序无关）。 */
export function matchPair(
  options: ActionLike[],
  picked: string[],
): string | null {
  if (picked.length !== 2 || picked[0] === picked[1]) return null;
  const key = [...picked].sort().join(",");
  for (const o of pairOptions(options)) {
    if ([...o.cards].sort().join(",") === key) return o.id;
  }
  return null;
}

export interface PairPickerCard {
  id: string;
  label: string;
}

/**
 * 渲染「两张手牌当【杀】」选择面板（响应窗口内）。
 *
 * 合法组合由引擎下发的 `pair:` 选项决定（端侧不判断技能/装备是否可用）。
 */
export function renderPairPicker(
  options: ActionLike[],
  hand: PairPickerCard[],
  picked: string[],
  opts: { title?: string; stateText?: string } = {},
): string {
  const legal = pairOptions(options);
  const usable = new Set(legal.flatMap((p) => p.cards));
  const rows = hand
    .filter((c) => usable.has(c.id))
    .map((c) => {
      const on = picked.includes(c.id);
      return (
        `<button type="button" class="btn small sgs-pick-target${on ? " selected" : ""}" ` +
        `data-pair-card="${escapeHtml(c.id)}">${escapeHtml(c.label)}</button>`
      );
    })
    .join("");
  const matched = matchPair(options, picked);
  const title = opts.title ?? "两张手牌当【杀】打出";
  const state = matched
    ? (opts.stateText ?? "可以确认（视为打出【杀】）")
    : picked.length === 0
      ? "请选择两张手牌"
      : picked.length === 1
        ? "再选一张"
        : "该组合不可用（换两张试试）";
  return (
    `<p class="muted">${escapeHtml(title)} · 合法组合 ${legal.length} 种</p>` +
    `<div class="sgs-zone-actions sgs-pick-targets">${rows || '<span class="muted">没有可选的两张牌</span>'}</div>` +
    `<p class="muted" data-pick-state="1">${escapeHtml(state)}</p>` +
    `<div class="sgs-zone-actions">` +
    `<button type="button" class="btn small" data-pair-confirm="1"${matched ? "" : " disabled"}>确认</button>` +
    `</div>`
  );
}

// ---------------------------------------------------------------------------
// 乱击（界袁绍）第二步「少选一个目标」：drop:<t> 折叠成「选择不打的目标 → 确认」
// ---------------------------------------------------------------------------

/** 解析 `drop:<seat>`（少选目标）。 */
export function dropOptions(options: ActionLike[]): Array<{ id: string; seat: number }> {
  const out: Array<{ id: string; seat: number }> = [];
  for (const o of options) {
    const m = /^drop:(\d+)$/.exec(o.id);
    if (m) out.push({ id: o.id, seat: Number(m[1]) });
  }
  return out;
}

/** 渲染「选择要少打的目标」（单选）面板；「不减少目标」等出口仍由外层按钮渲染。 */
export function renderDropPicker(
  options: ActionLike[],
  seats: Array<{ seat: number; label: string }>,
  picked: number | null,
): string {
  const legal = dropOptions(options);
  const bySeat = new Map(seats.map((s) => [s.seat, s.label]));
  const rows = legal
    .map((o) => {
      const on = picked === o.seat;
      const label = bySeat.get(o.seat) ?? `#${o.seat}`;
      return (
        `<button type="button" class="btn small sgs-pick-target${on ? " selected" : ""}" ` +
        `data-drop-seat="${o.seat}">少打 ${escapeHtml(label)}</button>`
      );
    })
    .join("");
  const matched = picked !== null && legal.some((o) => o.seat === picked);
  const state = matched ? "可以确认（本次万箭不打该角色）" : "请选择要少打的目标（或选择「不减少目标」）";
  return (
    `<p class="muted">乱击 · 可少选一个目标（剩余 ${legal.length} 名可选）</p>` +
    `<div class="sgs-zone-actions sgs-pick-targets">${rows || '<span class="muted">没有可少选的目标</span>'}</div>` +
    `<p class="muted" data-pick-state="1">${escapeHtml(state)}</p>` +
    `<div class="sgs-zone-actions">` +
    `<button type="button" class="btn small" data-drop-confirm="1"${matched ? "" : " disabled"}>确认</button>` +
    `</div>`
  );
}
