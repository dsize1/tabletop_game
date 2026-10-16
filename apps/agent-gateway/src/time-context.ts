//! 时间上下文（内部私有层）：向主持人提供「现在」与「距上次对话」的时间事实。
//!
//! 硬约束（详见 `docs/modules/agent-gateway.md`）：
//! - 只注入 **LLM 上下文**：不写入 `chat:history`、不出现在出站 QQ 消息里；
//! - 只给时间事实，不做「昨晚 / 该睡了」这类内容推断；
//! - 全部纯函数，时钟（`now`）可注入，便于单测。

/** 聊天时间所属时区（容器默认 UTC，故显式固定；`CHAT_TZ` 可覆盖）。 */
export const CHAT_TZ = process.env.CHAT_TZ ?? "Asia/Shanghai";

/** 同一「发言簇」的判定间隔：簇内只在首条打时间锚点。 */
export const CLUSTER_GAP_MS = 5 * 60 * 1000;

/** 话题切换边界标记（内部标记，不得出现在出站消息里）。 */
export const TOPIC_BOUNDARY_MARK = "--- 话题切换 ---";

const WEEKDAYS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"] as const;

/** 带时间戳的历史轮次（`chat:history` 中的存储结构）。 */
export interface TimedTurn {
  role: "user" | "assistant";
  content: string;
  /** 该条消息的写入时刻（epoch ms）；旧数据缺失 → 视为时间未知。 */
  ts?: number;
  /** 话题切换边界：本条起属于新话题（由决策模型判定后写入，长期复用）。 */
  boundary?: boolean;
}

export interface RenderedTurn {
  role: "user" | "assistant";
  content: string;
}

export interface TimeSnapshot {
  epochMs: number;
  /** 2026-02-14 */
  date: string;
  /** 周六 */
  weekday: string;
  /** 02-14 */
  mmdd: string;
  /** 09:12 */
  hhmm: string;
  /** UTC+8 */
  tzLabel: string;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** 目标时区下的本地日历字段。 */
function localParts(epochMs: number, tz: string) {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      hour12: false,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    })
      .formatToParts(new Date(epochMs))
      .map((p) => [p.type, p.value]),
  );
  return {
    year: Number(parts.year),
    month: Number(parts.month),
    day: Number(parts.day),
    // `hour12:false` 在部分环境把午夜格式化为 "24"。
    hour: Number(parts.hour) % 24,
    minute: Number(parts.minute),
    second: Number(parts.second),
  };
}

/** 目标时区相对 UTC 的偏移（分钟）。 */
function tzOffsetMinutes(epochMs: number, tz: string): number {
  const p = localParts(epochMs, tz);
  const asUtc = Date.UTC(p.year, p.month - 1, p.day, p.hour, p.minute, p.second);
  return Math.round((asUtc - epochMs) / 60000);
}

function offsetLabel(minutes: number): string {
  const sign = minutes < 0 ? "-" : "+";
  const abs = Math.abs(minutes);
  const h = Math.floor(abs / 60);
  const m = abs % 60;
  return `UTC${sign}${h}${m ? `:${pad(m)}` : ""}`;
}

/** 取某时刻在目标时区的时间快照。 */
export function nowInTz(now: number = Date.now(), tz: string = CHAT_TZ): TimeSnapshot {
  const p = localParts(now, tz);
  return {
    epochMs: now,
    date: `${p.year}-${pad(p.month)}-${pad(p.day)}`,
    weekday: WEEKDAYS[new Date(Date.UTC(p.year, p.month - 1, p.day)).getUTCDay()]!,
    mmdd: `${pad(p.month)}-${pad(p.day)}`,
    hhmm: `${pad(p.hour)}:${pad(p.minute)}`,
    tzLabel: offsetLabel(tzOffsetMinutes(now, tz)),
  };
}

/**
 * 间隔的紧凑表示（纯数字，不含「昨晚 / 早上」这类语义推断）：
 * `< 1min → 刚刚`、`< 60min → 32m`、`< 24h → 14h32m`、`≥ 24h → 1d14h`。
 */
export function formatGap(ms: number): string {
  if (!Number.isFinite(ms) || ms < 60_000) return "刚刚";
  const totalMin = Math.floor(ms / 60_000);
  if (totalMin < 60) return `${totalMin}m`;
  const totalHour = Math.floor(totalMin / 60);
  const min = totalMin % 60;
  if (totalHour < 24) return `${totalHour}h${min ? `${min}m` : ""}`;
  const day = Math.floor(totalHour / 24);
  const hour = totalHour % 24;
  return `${day}d${hour ? `${hour}h` : ""}`;
}

/** 时间块（作为独立 system 条目注入 LLM 上下文；不落库、不出站）。 */
export function formatTimeBlock(
  now: number = Date.now(),
  lastTs?: number,
  tz: string = CHAT_TZ,
): string {
  const s = nowInTz(now, tz);
  const head = `[时间] 现在 ${s.date} ${s.weekday} ${s.hhmm} (${s.tzLabel})`;
  if (typeof lastTs !== "number" || !Number.isFinite(lastTs)) return head;
  return `${head}；距上次对话 ${formatGap(now - lastTs)}`;
}

/** 是否给本条打时间锚点：首条（或前一条时间未知），或与上一条间隔 ≥ `CLUSTER_GAP_MS`。 */
function shouldAnchor(prevTs: number | undefined, ts: number | undefined): boolean {
  if (typeof ts !== "number" || !Number.isFinite(ts)) return false;
  if (typeof prevTs !== "number") return true;
  return ts - prevTs >= CLUSTER_GAP_MS;
}

/**
 * 渲染历史为「送 LLM 的 messages」：簇首加 `[MM-DD HH:mm]` 锚点，带边界的轮次前插话题切换标记。
 * 注意：只影响本次上下文，`content` 本身不被改写、不入库、不出站。
 */
export function renderHistory(
  turns: readonly TimedTurn[],
  opts: { tz?: string } = {},
): RenderedTurn[] {
  const tz = opts.tz ?? CHAT_TZ;
  const out: RenderedTurn[] = [];
  let prevTs: number | undefined;
  for (const t of turns) {
    const lines: string[] = [];
    if (t.boundary) lines.push(TOPIC_BOUNDARY_MARK);
    // 锚点与消息同一行（边界标记单独成行）
    let anchor = "";
    if (shouldAnchor(prevTs, t.ts)) {
      const s = nowInTz(t.ts!, tz);
      anchor = `[${s.mmdd} ${s.hhmm}] `;
    }
    lines.push(`${anchor}${t.content}`);
    out.push({ role: t.role, content: lines.join("\n") });
    if (typeof t.ts === "number" && Number.isFinite(t.ts)) prevTs = t.ts;
  }
  return out;
}

/** 最近一条带时间戳的轮次时刻（时间未知 → undefined）。 */
export function lastTurnTs(turns: readonly TimedTurn[]): number | undefined {
  for (let i = turns.length - 1; i >= 0; i--) {
    const ts = turns[i]!.ts;
    if (typeof ts === "number" && Number.isFinite(ts)) return ts;
  }
  return undefined;
}

/** 距上次对话的间隔（毫秒）；无可用时间戳 → undefined。 */
export function chatGapMs(turns: readonly TimedTurn[], now: number = Date.now()): number | undefined {
  const lastTs = lastTurnTs(turns);
  if (lastTs === undefined) return undefined;
  return Math.max(0, now - lastTs);
}

/**
 * 「话题门控」：间隔是否已跨越阈值（`gateMs ≤ 0` 表示整体关闭门控）。
 * 命中时：① 追加 `topic_switch` 判定；② 丢弃已失去语境的未决意图确认（pending）。
 */
export function isStaleGap(gapMs: number | undefined, gateMs: number): boolean {
  if (!Number.isFinite(gateMs) || gateMs <= 0) return false;
  return typeof gapMs === "number" && Number.isFinite(gapMs) && gapMs >= gateMs;
}

/** 最近一条用户消息内容（供话题切换判定当 `previous_message`）。 */
export function lastUserContent(turns: readonly TimedTurn[]): string | undefined {
  for (let i = turns.length - 1; i >= 0; i--) {
    const t = turns[i]!;
    if (t.role === "user") return t.content;
  }
  return undefined;
}
