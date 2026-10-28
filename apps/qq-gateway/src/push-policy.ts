// qq.push 出站投递策略（纯函数，便于单测，不触网/不碰 Redis）：
// 「哪些发送失败值得重试」「哪些条目重投也不可能成功，应直接 ACK 丢弃」。
//
// 依据：
// - 被动回复窗口 5 分钟 / 每条 5 次（docs/conventions/external-constraints.md，已验证）
// - 实测业务错误码：40034024 msg_id 无效或越权、11321 group_id 无效
//   （docs/modules/qq-gateway.md「出站消费」）
//
// 背景事故（2026-10-29）：整链停机后重启，qq.push PEL 里 12 天前的被动回复被重投，
// 每次都 40034024 且「保留待重试」→ 每次重启刷一屏失败日志，条目永久占位 PEL。

/** 群聊被动回复有效期：5 分钟（超窗后 QQ 必定拒绝该 msg_id）。 */
export const PASSIVE_REPLY_WINDOW_MS = 5 * 60 * 1000;

/** 重试无望的业务错误码。 */
export const PERMANENT_BIZ_CODES: ReadonlySet<number> = new Set([
  40034024, // 请求参数 msg_id 无效或越权（被动回复超窗/已用尽次数）
  11321, // 请求 Query 参数 group_id 无效（群 openid 错误 / 机器人不在群）
]);

export interface PushEventLike {
  group_id?: string;
  content?: string;
  reply_msg_id?: string | null;
  msg_seq?: number;
}

interface ApiFailure {
  httpStatus?: number;
  bizCode?: number;
  bizMessage?: string;
  message?: string;
}

export function asApiFailure(err: unknown): ApiFailure {
  return typeof err === "object" && err !== null ? (err as ApiFailure) : {};
}

/** 重试也无望：永久性业务码，或 429（频控，可稍后重试）之外的 4xx。 */
export function isPermanentFailure(err: unknown): boolean {
  const { httpStatus, bizCode } = asApiFailure(err);
  if (bizCode !== undefined && PERMANENT_BIZ_CODES.has(bizCode)) return true;
  return (
    typeof httpStatus === "number" && httpStatus >= 400 && httpStatus < 500 && httpStatus !== 429
  );
}

/** 单行失败摘要，用于丢弃日志。 */
export function describeFailure(err: unknown): string {
  const { httpStatus, bizCode, bizMessage, message } = asApiFailure(err);
  const parts = [
    httpStatus ? `http=${httpStatus}` : "",
    bizCode ? `code=${bizCode}` : "",
    bizMessage ?? message ?? "",
  ];
  return parts.filter(Boolean).join(" ") || String(err);
}

/** 从 Stream ID（`<ms>-<seq>`）取写入时刻；无法解析返回 0（视作极旧）。 */
export function entryTimestamp(id: string): number {
  const ms = Number(id.split("-")[0]);
  return Number.isFinite(ms) && ms > 0 ? ms : 0;
}

/** 条目写入时刻距 `nowMs` 的毫秒数。 */
export function entryAgeMs(id: string, nowMs: number = Date.now()): number {
  return nowMs - entryTimestamp(id);
}

/**
 * 该条目是否已注定失败：被动回复（带 `reply_msg_id`）早在窗口外 + 已超窗。
 * 主动消息（无 `reply_msg_id`）不受窗口约束，返回 false。
 */
export function isExpiredReply(
  id: string,
  event: PushEventLike,
  nowMs: number = Date.now(),
): boolean {
  if (!event.reply_msg_id) return false;
  return entryAgeMs(id, nowMs) > PASSIVE_REPLY_WINDOW_MS;
}
