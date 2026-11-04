// 历史 → 特效 cue（纯函数，P0）：不依赖后端字段，只解析 `public_board.history`。
//
// 设计：
// - `prev/next` 为同一房间的两次视图快照；取 `seq > prev 末条 seq` 的增量；
// - 文本规则优先（引擎 push_log 文案），HP/体力差分兜底（保证扣血必有受击）；
// - 同目标同类型连续 cue 合并（×N）；超过上限只保留最近 N 条；
// - 解析失败静默跳过，绝不抛错（特效是装饰层）。

import type { SgsHistoryEntry, SgsSeat, SgsViewData } from "./types.js";

export type FxKind =
  | "slash"
  | "dodge"
  | "peach"
  | "wine"
  | "thunder"
  | "fire"
  | "chain"
  | "heal"
  | "lose";

export interface FxCue {
  /** 来源 history seq（同一 seq 只消费一次）。 */
  seq: number;
  kind: FxKind;
  /** 目标座位（null = 无座位/全屏）。 */
  seat: number | null;
  /** 合并次数（同目标同类型连续事件）。 */
  count: number;
  /** 伤害/回复点数。 */
  amount?: number;
  /** 加强变体（雷击/闪电/火攻）。 */
  variant?: "big";
  /** 短文案（调试/字幕用，截断）。 */
  text?: string;
}

export interface FxContext {
  seats: SgsSeat[];
}

const MAX_CUES = 12;

function seatOfPlayer(seats: SgsSeat[], pid: string | null | undefined): number | null {
  if (!pid) return null;
  const hit = seats.find((s) => s.player_id === pid);
  return hit ? hit.seat : null;
}

function seatOfName(seats: SgsSeat[], name: string | null | undefined): number | null {
  const n = (name ?? "").trim();
  if (!n) return null;
  const hit = seats.find((s) => (s.general_name ?? "").trim() === n);
  return hit ? hit.seat : null;
}

function seatOfAny(
  seats: SgsSeat[],
  raw: string | null | undefined,
  actor?: string | null,
): number | null {
  return seatOfPlayer(seats, raw) ?? seatOfName(seats, raw) ?? seatOfPlayer(seats, actor);
}

function shortText(text: string): string {
  const t = text.replace(/\s+/g, " ").trim();
  return t.length > 42 ? `${t.slice(0, 41)}…` : t;
}

/** 解析一条历史（可产生多个 cue：如铁索传导先链缚再受击）。 */
function parseEntry(
  entry: SgsHistoryEntry,
  seats: SgsSeat[],
  chain: { remaining: number; thunderHint: boolean },
): FxCue[] {
  const text = entry.text ?? "";
  const actorSeat = seatOfPlayer(seats, entry.actor);
  const out: FxCue[] = [];
  const base = { seq: entry.seq, count: 1, text: shortText(text) };

  // 铁索连环：直接选目标 / 属性伤害传导。
  if (/^铁索连环：/.test(text)) {
    const countMatch = /传导至\s*(\d+)\s*名/.exec(text);
    if (countMatch) chain.remaining = Math.max(chain.remaining, Number(countMatch[1]) || 1);
    const targetPart = /^铁索连环：对\s*(.+?)\s*结算/.exec(text)?.[1];
    if (targetPart) {
      for (const name of targetPart.split(/[、,，\s]+/).filter(Boolean)) {
        const seat = seatOfName(seats, name);
        if (seat !== null) out.push({ ...base, kind: "chain", seat });
      }
    }
    if (out.length > 0 || countMatch) return out;
  }

  // 伤害：`<pid> 受到 N 点<普通|火焰|雷电>伤害（体力 X）`
  const dmg = /^(\S+)\s*受到\s*(\d+)\s*点(普通|火焰|雷电)伤害/.exec(text);
  if (dmg) {
    const seat = seatOfAny(seats, dmg[1], entry.actor);
    const amount = Number(dmg[2]) || 1;
    const type = dmg[3];
    const kind: FxKind = type === "火焰" ? "fire" : type === "雷电" ? "thunder" : "slash";
    const big = kind === "thunder" && chain.thunderHint;
    if (kind === "thunder") chain.thunderHint = false;
    if (chain.remaining > 0 && kind !== "slash") {
      if (seat !== null) out.push({ ...base, kind: "chain", seat });
      chain.remaining -= 1;
    }
    out.push({ ...base, kind, seat, amount, variant: big ? "big" : undefined });
    return out;
  }

  // 闪：响应打出【闪…】。
  if (/打出【闪|使用【闪/.test(text)) {
    out.push({ ...base, kind: "dodge", seat: actorSeat });
    return out;
  }

  // 桃（回合内使用 / 濒死救援 / 自救）。
  if (/使用【桃/.test(text)) {
    out.push({ ...base, kind: "peach", seat: actorSeat });
    return out;
  }

  // 酒（回合内使用 / 酒诗等）。
  if (/使用【酒|视为使用【酒/.test(text)) {
    out.push({ ...base, kind: "wine", seat: actorSeat });
    return out;
  }

  // 雷击/闪电提示：下一条雷电伤害加强。
  if (/雷击：|【闪电】/.test(text)) {
    chain.thunderHint = true;
    return out;
  }

  // 回血（非桃）：`<name> 回复 N 点体力`。
  const heal = /(\S+)\s*回复\s*(\d+)\s*点体力/.exec(text);
  if (heal) {
    const seat = seatOfAny(seats, heal[1], entry.actor);
    out.push({ ...base, kind: "heal", seat, amount: Number(heal[2]) || 1 });
    return out;
  }

  // 失去体力（非伤害）：`<name> 失去 N 点体力`。
  const lose = /(\S+)\s*失去\s*(\d+)\s*点体力/.exec(text);
  if (lose) {
    const seat = seatOfAny(seats, lose[1], entry.actor);
    out.push({ ...base, kind: "lose", seat, amount: Number(lose[2]) || 1 });
    return out;
  }

  return out;
}

function mergeCues(cues: FxCue[]): FxCue[] {
  const out: FxCue[] = [];
  for (const cue of cues) {
    const prev = out[out.length - 1];
    if (prev && prev.kind === cue.kind && prev.seat === cue.seat && prev.variant === cue.variant) {
      prev.count += cue.count;
      prev.amount = (prev.amount ?? 0) + (cue.amount ?? 0);
      prev.seq = cue.seq;
      prev.text = cue.text;
      continue;
    }
    out.push({ ...cue });
  }
  return out;
}

/** 前后快照差分兜底：文本规则漏掉的扣血/回血也要有反馈。 */
function diffFallback(prev: SgsViewData, next: SgsViewData, cues: FxCue[]): FxCue[] {
  const prevSeats = prev.public_board?.seats ?? [];
  const nextSeats = next.public_board?.seats ?? [];
  const out = [...cues];
  const has = (seat: number, kinds: FxKind[]): boolean =>
    out.some((c) => c.seat === seat && kinds.includes(c.kind));
  for (const seat of nextSeats) {
    const before = prevSeats[seat.seat];
    if (!before) continue;
    if (seat.hp < before.hp) {
      if (!has(seat.seat, ["slash", "thunder", "fire"])) {
        out.push({
          seq: (next.public_board?.history?.at(-1)?.seq ?? 0) + 0.001 * (seat.seat + 1),
          kind: "slash",
          seat: seat.seat,
          count: 1,
          amount: before.hp - seat.hp,
          text: `体力 -${before.hp - seat.hp}`,
        });
      }
    } else if (seat.hp > before.hp && !has(seat.seat, ["peach", "heal"])) {
      out.push({
        seq: (next.public_board?.history?.at(-1)?.seq ?? 0) + 0.001 * (seat.seat + 1),
        kind: "heal",
        seat: seat.seat,
        count: 1,
        amount: seat.hp - before.hp,
        text: `体力 +${seat.hp - before.hp}`,
      });
    }
  }
  return out;
}

/**
 * 从前后快照生成特效队列。
 *
 * @param limit 最多保留最近 N 条（默认 12；超过时丢弃更早的）。
 */
export function buildFxCues(
  prev: SgsViewData | null | undefined,
  next: SgsViewData | null | undefined,
  limit = MAX_CUES,
): FxCue[] {
  try {
    if (!prev || !next) return [];
    const prevHistory = prev.public_board?.history ?? [];
    const nextHistory = next.public_board?.history ?? [];
    const lastSeq = prevHistory.length ? prevHistory[prevHistory.length - 1].seq : 0;
    // 新局 seq 回绕：不做回放。
    if (nextHistory.length === 0) return [];
    if (lastSeq > nextHistory[nextHistory.length - 1].seq) return [];
    const seats = next.public_board?.seats ?? [];
    const chain = { remaining: 0, thunderHint: false };
    const cues: FxCue[] = [];
    for (const entry of nextHistory) {
      if (entry.seq <= lastSeq) continue;
      cues.push(...parseEntry(entry, seats, chain));
    }
    const withFallback = diffFallback(prev, next, cues);
    const merged = mergeCues(withFallback);
    const cap = Math.max(1, Math.floor(limit));
    return merged.slice(-cap);
  } catch {
    return [];
  }
}

/** 同一次 `applyState` 的 history 末 seq（用于推进消费游标）。 */
export function historyLastSeq(data: SgsViewData | null | undefined): number {
  const h = data?.public_board?.history;
  return h && h.length ? h[h.length - 1].seq : 0;
}
