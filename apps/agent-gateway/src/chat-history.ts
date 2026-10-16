//! 聊天历史（Redis 热缓存，最近 N 条；全量归档由 bot-adapter 写 SQLite `group_messages`）。
//!
//! 条目随消息一起存 `ts`（写入时刻）与 `boundary`（话题切换边界），供 `time-context.ts`
//! 渲染时间锚点 / 边界标记。两者都是**内部元数据**：只进 LLM 上下文，不进出站消息。
//!
//! 存储接口最小化（`ListStore`），便于单测注入内存实现。

import type { Intent } from "./decision.js";
import type { TimedTurn } from "./time-context.js";

/** 聊天历史热缓存的条数上限（超出即从头部裁剪）。 */
export const CHAT_HISTORY_LIMIT = Number(process.env.CHAT_HISTORY_LIMIT ?? 20);

/** 聊天会话键：按 `(群, 用户)` 隔离（ADR-0007 决策 3）。 */
export const chatHistoryKey = (groupId: string, openid: string) =>
  `chat:history:${groupId}:${openid}`;

/** 最小列表存储接口（ioredis 结构兼容；测试可注入内存实现）。 */
export interface ListStore {
  lrange(key: string, start: number, stop: number): Promise<string[]>;
  rpush(key: string, ...values: string[]): Promise<unknown>;
  ltrim(key: string, start: number, stop: number): Promise<unknown>;
}

function parseTurn(raw: string): TimedTurn | null {
  let obj: unknown;
  try {
    obj = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!obj || typeof obj !== "object") return null;
  const t = obj as Partial<TimedTurn>;
  if (typeof t.content !== "string" || !t.content) return null;
  const turn: TimedTurn = {
    role: t.role === "assistant" ? "assistant" : "user",
    content: t.content,
  };
  // 向后兼容：旧条目无 ts / boundary → 视为时间未知、无边界
  if (typeof t.ts === "number" && Number.isFinite(t.ts)) turn.ts = t.ts;
  if (t.boundary === true) turn.boundary = true;
  return turn;
}

/** 读取最近 N 条聊天历史（旧格式条目自动降级为「时间未知」）。 */
export async function getChatHistory(
  store: ListStore,
  groupId: string,
  openid: string,
  limit: number = CHAT_HISTORY_LIMIT,
): Promise<TimedTurn[]> {
  const raw = await store.lrange(chatHistoryKey(groupId, openid), -limit, -1);
  const out: TimedTurn[] = [];
  for (const r of raw) {
    const t = parseTurn(r);
    if (t) out.push(t);
  }
  return out;
}

/** 追加一条聊天历史（`ts` 默认当前时刻；`boundary` 仅对当前这一轮生效）。 */
export async function appendChatHistory(
  store: ListStore,
  groupId: string,
  openid: string,
  role: "user" | "assistant",
  content: string,
  meta: { ts?: number; boundary?: boolean } = {},
  limit: number = CHAT_HISTORY_LIMIT,
): Promise<void> {
  const entry: TimedTurn = {
    role,
    content,
    ts: meta.ts ?? Date.now(),
    ...(meta.boundary ? { boundary: true } : {}),
  };
  const key = chatHistoryKey(groupId, openid);
  await store.rpush(key, JSON.stringify(entry));
  await store.ltrim(key, -limit, -1);
}

// ==================== 待落位的话题边界 ====================

/**
 * 话题边界只在 chat 分支写入 `chat:history`；若检出切换的那条消息被路由到 game/dev，
 * 边界先暂存到 Redis，由「空档后第一条进入聊天历史的用户消息」取用并落位。
 * 因 `gap` 基于 `chat:history` 最后一条的 `ts`，未写入时门控会持续命中而自愈。
 */
/**
 * 阶段裁剪（ADR-0007 决策 6 / a1）：只保留最近一条 `boundary`（含）之后的轮次。
 * `boundary` 标记「本条起属于新话题」，故裁剪 = 从最后一条带 `boundary` 的轮次到末尾。
 * 无任何 boundary（新会话 / 无切换）→ 原样返回。纯函数，便于单测。
 */
export function cropToCurrentPhase(turns: readonly TimedTurn[]): TimedTurn[] {
  for (let i = turns.length - 1; i >= 0; i--) {
    if (turns[i]!.boundary) return turns.slice(i);
  }
  return turns.slice(0);
}

export const pendingBoundaryKey = (groupId: string, openid: string) =>
  `chat:boundary:${groupId}:${openid}`;

/** 待落位边界的存活时长（秒）：只需挈过门控窗口，保守取 1 小时。 */
export const PENDING_BOUNDARY_TTL_SEC = 3600;

/** 最小 KV 存储接口（ioredis 兼容；测试可注入内存实现）。 */
export interface KVStore {
  get(key: string): Promise<string | null>;
  set(key: string, value: string, mode: "EX", ttl: number): Promise<unknown>;
  del(key: string): Promise<unknown>;
}

/** 边界去向：`history`=本轮写的 chat 消息直接带边界；`stash`=本轮不写 history，暂存等下一次；`none`=无边界。 */
export type BoundaryDisposition = "history" | "stash" | "none";

/** 根据意图决定边界去向（纯函数，便于单测）。 */
export function boundaryDisposition(boundary: boolean, intent: Intent): BoundaryDisposition {
  if (!boundary) return "none";
  return intent === "chat" ? "history" : "stash";
}

/** 暂存一个待落位边界。 */
export async function markPendingBoundary(
  store: KVStore,
  groupId: string,
  openid: string,
  ttlSec: number = PENDING_BOUNDARY_TTL_SEC,
): Promise<void> {
  await store.set(
    pendingBoundaryKey(groupId, openid),
    "1",
    "EX",
    Math.max(1, Math.floor(ttlSec)),
  );
}

/** 取用并清除待落位边界（返回是否存在）。 */
export async function consumePendingBoundary(
  store: KVStore,
  groupId: string,
  openid: string,
): Promise<boolean> {
  const key = pendingBoundaryKey(groupId, openid);
  const raw = await store.get(key);
  if (raw === null) return false;
  await store.del(key);
  return true;
}

/** 丢弃待落位边界（门控重新判定时以本次为准）。 */
export async function clearPendingBoundary(
  store: KVStore,
  groupId: string,
  openid: string,
): Promise<void> {
  await store.del(pendingBoundaryKey(groupId, openid));
}
