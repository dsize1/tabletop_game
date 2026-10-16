//! 意图确认（中带置信度的人工二次确认）状态：Redis 暂存 + 是/否词表匹配。
//! 「确认」= 用户对决策模型给出的候选意图做是与否的裁定，不与 dev 的 `#确认`（计划确认）混用。
import type { Intent } from "./decision.js";

/** 最小存储接口（ioredis Redis 结构兼容；测试可注入内存实现）。 */
export interface ConfirmStore {
  get(key: string): Promise<string | null>;
  set(key: string, value: string, mode: "EX", ttl: number): Promise<unknown>;
  del(key: string): Promise<unknown>;
}

/** 未决的候选意图（中带置信度，等待人工 y/n）。 */
export interface PendingIntent {
  intent: Intent;
  confidence: number;
  /** 触发确认的原始消息（确认=是时按它执行，而非按「y」执行）。 */
  content: string;
  /** 原问题的 QQ message_id（关联 id）：用于把「原问题→确认提示」与「y/n→执行答复」两轮日志串起来。 */
  cid: string;
  /** 是否已追问过一次（追问后仍非 y/n 判 n）。 */
  reasked: boolean;
  /** 写入时间（epoch ms，便于异常记录）。 */
  ts: number;
}

/** 未决确认的默认存活时长（秒）：对齐群被动回复窗口 5 分钟（external-constraints 已验证）。 */
export const DEFAULT_CONFIRM_TTL_SEC = 300;

/** 未决确认按 (群, 发送者 openid) 隔离——同一个人覆盖，不同人互不影响。 */
export const intentPendingKey = (groupId: string, senderId: string) =>
  `intent:pending:${groupId}:${senderId}`;

export async function loadPending(
  store: ConfirmStore,
  groupId: string,
  senderId: string,
): Promise<PendingIntent | null> {
  const raw = await store.get(intentPendingKey(groupId, senderId));
  if (!raw) return null;
  try {
    const p = JSON.parse(raw) as PendingIntent;
    if (!p || typeof p.intent !== "string" || typeof p.content !== "string") return null;
    return { ...p, reasked: p.reasked === true };
  } catch {
    return null;
  }
}

export async function savePending(
  store: ConfirmStore,
  groupId: string,
  senderId: string,
  pending: PendingIntent,
  ttlSec: number = DEFAULT_CONFIRM_TTL_SEC,
): Promise<void> {
  await store.set(
    intentPendingKey(groupId, senderId),
    JSON.stringify(pending),
    "EX",
    Math.max(1, Math.floor(ttlSec)),
  );
}

export async function clearPending(
  store: ConfirmStore,
  groupId: string,
  senderId: string,
): Promise<void> {
  await store.del(intentPendingKey(groupId, senderId));
}

const YES_WORDS = new Set([
  "y", "yes", "yeah", "yep", "ok", "okay",
  "是", "对", "对的", "是的", "确认", "确定", "好", "好的", "嗯", "可以",
]);
const NO_WORDS = new Set([
  "n", "no", "nope",
  "否", "不", "不是", "不对", "不要", "不用", "取消", "算了",
]);

/**
 * 匹配确认回复：仅当整条消息（trim + 去尾部中英标点后小写）恰为 y/n 词才判定，否则 null。
 * 严格整句匹配，避免「是，继续…」这类长句被误判为确认。
 */
export function matchConfirmReply(content: string): "yes" | "no" | null {
  const s = content.trim().replace(/[。.!！?？~～,，、;；:：'"“”‘’]+$/u, "").toLowerCase();
  if (!s) return null;
  if (YES_WORDS.has(s)) return "yes";
  if (NO_WORDS.has(s)) return "no";
  return null;
}

/**
 * 异常记录：未决确认被覆盖 / 追问后仍非 y/n 等「非预期路径」。
 * 当前落结构化日志（stderr → agent-gateway.err.log），便于事后统计阈值是否需要复核。
 */
export function recordAnomaly(reason: string, detail: Record<string, unknown>): void {
  console.error(`[意图确认][异常] ${reason} ${JSON.stringify(detail)}`);
}
