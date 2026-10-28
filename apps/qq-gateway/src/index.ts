import "dotenv/config";
import { QQBot } from "@tencent-connect/qqbot-nodejs";
import { Redis } from "ioredis";
import {
  describeFailure,
  entryAgeMs,
  isExpiredReply,
  isPermanentFailure,
  type PushEventLike,
} from "./push-policy.js";

const appId = process.env.QQBOT_APP_ID;
const appSecret = process.env.QQBOT_APP_SECRET;
const redisUrl = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";

if (!appId || !appSecret) {
  console.error(
    "[qq-gateway] 缺少 QQBOT_APP_ID / QQBOT_APP_SECRET，请在 .env 中配置（参考 .env.example）",
  );
  process.exit(1);
}

const redis = new Redis(redisUrl);
redis.on("error", (err) => console.error("[qq-gateway] redis error:", err.message));

// SDK 自动处理：Access Token 获取/刷新、WebSocket 网关连接、心跳、断线重连
const bot = new QQBot({ appId, appSecret, logger: console });

const RAW_STREAM = "qq.raw";
const PUSH_STREAM = "qq.push";
const GROUP = "qq-gateway";
const CONSUMER = "qq-gateway-1";

// 投递策略（超窗丢弃 / 永久失败丢弃）见 push-policy.ts
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));


// ===== 入站：群事件 → qq.raw =====
bot.on("ready", () => console.log("[qq-gateway] ✅ WebSocket 网关已连接"));

bot.on("message", async (_ctx, msg) => {
  if (msg.kind !== "group") return; // 当前仅处理群消息

  // 原始事件完整透传：eventType 区分 @消息 / 全量消息
  const event = {
    eventType: msg.rawEventType, // GROUP_AT_MESSAGE_CREATE | GROUP_MESSAGE_CREATE
    raw: msg.raw, // 原始平台事件对象
    receivedAt: Date.now(),
  };

  try {
    await redis.xadd(RAW_STREAM, "MAXLEN", "~", "1000", "*", "event", JSON.stringify(event));
    console.log(`[qq-gateway] → ${RAW_STREAM} [${msg.rawEventType}] group=${msg.groupOpenid}`);
  } catch (err) {
    console.error("[qq-gateway] 写入 qq.raw 失败:", err);
  }
});

bot.on("error", (err) => console.error("[qq-gateway] error:", err));

// ===== 出站：qq.push → 发群消息 =====
async function ensureGroup() {
  try {
    await redis.xgroup("CREATE", PUSH_STREAM, GROUP, "0", "MKSTREAM");
  } catch (err) {
    const msg = String((err as Error)?.message ?? err);
    if (!msg.includes("BUSYGROUP")) throw err;
  }
}

/** 发送一条群消息：有 reply_msg_id 走被动回复（显式 msg_seq 递增），否则主动发送。返回发送结果。 */
async function sendGroupMessage(event: {
  group_id: string;
  content: string;
  reply_msg_id?: string | null;
  msg_seq?: number;
}) {
  if (event.reply_msg_id) {
    // 被动回复：显式 msg_id + msg_seq（SDK sendText 的 msg_seq 为随机数，不满足多条递增）
    return await bot.send({
      target: { scope: "group", targetId: event.group_id, msgId: event.reply_msg_id },
      content: event.content,
      extra: { msg_seq: event.msg_seq ?? 1 },
    });
  }
  // 主动消息：无 msg_id，走 sendText
  return await bot.sendText({ scope: "group", targetId: event.group_id }, event.content);
}

/** ACK 掉一条永远不可能发送成功的条目（重投只会永远失败）。 */
async function dropEntry(id: string, reason: string) {
  await redis.xack(PUSH_STREAM, GROUP, id);
  console.warn(`[qq-gateway] 丢弃 ${PUSH_STREAM} [${id}]：${reason}`);
}

async function processPushEntries(results: unknown, now: () => number = Date.now) {
  for (const [, entries] of results as [string, [string, string[]][]][]) {
    for (const [id, fields] of entries) {
      // fields: [field1, value1, field2, value2, ...]
      const map: Record<string, string> = {};
      for (let i = 0; i < fields.length; i += 2) map[fields[i]] = fields[i + 1];

      const eventJson = map["event"];
      if (!eventJson) {
        // 结构损坏的条目重投永远不会成功，ACK 掉，别让它永久赖在 PEL 里
        await dropEntry(id, "缺少 event 字段");
        continue;
      }

      let event: PushEventLike & { group_id: string; content: string };
      try {
        event = JSON.parse(eventJson);
      } catch {
        await dropEntry(id, "event 不是合法 JSON");
        continue;
      }

      // 被动回复已超窗：QQ 必拒，直接丢弃（停机积压重投时最常见）
      const nowMs = now();
      if (isExpiredReply(id, event, nowMs)) {
        const ageS = Math.round(entryAgeMs(id, nowMs) / 1000);
        await dropEntry(id, `被动回复已超窗（滞留 ${ageS}s > 300s）`);
        continue;
      }

      try {
        const res = await sendGroupMessage(event);
        await redis.xack(PUSH_STREAM, GROUP, id);
        console.log(`[qq-gateway] ← ${PUSH_STREAM} 已发送 [${id}] msg_id=${res?.id ?? "?"}`);
      } catch (err) {
        if (isPermanentFailure(err)) {
          // 永久失败（超窗/越权/群无效）：重试只会永远失败并刷日志 → ACK 丢弃并告警
          await dropEntry(id, `永久失败，不再重试（${describeFailure(err)}）`);
          continue;
        }
        console.error(`[qq-gateway] 发送失败 [${id}]，保留待重试:`, err);
        await sleep(500);
      }
    }
  }
}

async function consumePush() {
  await ensureGroup();
  console.log(`[qq-gateway] 出站消费者就绪：${PUSH_STREAM}`);

  // 启动时重投 PEL 中未 ACK 的历史消息
  try {
    const pending = (await redis.xreadgroup(
      "GROUP", GROUP, CONSUMER,
      "COUNT", 50,
      "STREAMS", PUSH_STREAM, "0",
    )) as unknown;
    if (pending) {
      await processPushEntries(pending);
      console.log("[qq-gateway] PEL 重投完成");
    }
  } catch (err) {
    console.error("[qq-gateway] 重投 PEL 失败:", err);
  }

  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      const results = (await redis.xreadgroup(
        "GROUP", GROUP, CONSUMER,
        "COUNT", 50,
        "BLOCK", 2000,
        "STREAMS", PUSH_STREAM, ">",
      )) as unknown;

      if (results) await processPushEntries(results);
    } catch (err) {
      console.error("[qq-gateway] 消费 qq.push 失败:", err);
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
}

// 优雅退出：SIGINT / SIGTERM → abort → start() 返回 → 关闭 Redis
const ac = new AbortController();
const shutdown = () => ac.abort();
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

// 出站消费者后台运行
void consumePush().catch((err) => console.error("[qq-gateway] 出站循环崩溃:", err));

try {
  await bot.start(ac.signal);
} catch (err) {
  console.error("[qq-gateway] 致命错误:", err);
  process.exitCode = 1;
} finally {
  await redis.quit().catch(() => {});
}
