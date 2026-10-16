//! 对话归档（chat + game 分支 → Redis Stream `chat.log` → server 落 SQLite `chat_history`）。
//!
//! per-user 对话是人物画像蒸馏的数据源（ADR-0007 决策 12/13）。写入为 at-least-once，
//! 落库侧按 `event_id` 幂等去重。**dev 分支不入归档**；确认提示 / y/n 也不入
//! （与 `chat:history` 的语义保持一致）。
//!
//! 写入失败仅告警，不影响主流程（对话归档非关键路径）。

import { randomUUID } from "node:crypto";

export const CHAT_LOG_STREAM = "chat.log";

/** `chat.log` 保留上限（条）。与其它 Stream 同量级。 */
export const CHAT_LOG_MAXLEN = Number(process.env.CHAT_LOG_MAXLEN ?? 1000);

export type ChatLogRole = "user" | "assistant";

/** 最小 Stream 追加接口（ioredis 兼容；测试可注入内存实现）。 */
export interface StreamAppender {
  xadd(key: string, ...args: (string | number)[]): Promise<unknown>;
}

export interface ChatLogMessage {
  openid: string;
  groupId: string;
  role: ChatLogRole;
  content: string;
}

/**
 * 追加一条 per-user 对话到 `chat.log`。失败仅告警（不抛错），保证非关键路径不阻塞。
 */
export async function emitChatLog(
  redis: StreamAppender,
  msg: ChatLogMessage,
): Promise<void> {
  if (!msg.content) return;
  const event = {
    event_id: randomUUID(),
    openid: msg.openid,
    group_id: msg.groupId,
    role: msg.role,
    content: msg.content,
    created_at: new Date().toISOString(),
  };
  try {
    await redis.xadd(
      CHAT_LOG_STREAM,
      "MAXLEN",
      "~",
      CHAT_LOG_MAXLEN,
      "*",
      "event",
      JSON.stringify(event),
    );
  } catch (err) {
    console.error("[agent-gateway] 写 chat.log 失败（忽略）:", err);
  }
}
