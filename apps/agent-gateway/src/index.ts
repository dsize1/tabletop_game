import "dotenv/config";
import { Redis } from "ioredis";
import { randomUUID } from "node:crypto";
import { decideIntent, extractHand } from "./decision.js";
import { chatReply } from "./chat.js";
import { GameEngineClient } from "./mcp.js";

const redisUrl = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const INTERNAL_STREAM = "internal";
const PUSH_STREAM = "qq.push";
const GROUP = "agent-gateway";
const CONSUMER = "agent-gateway-1";

// dev 白名单（member_openid 逗号分隔）
const DEV_WHITELIST = new Set(
  (process.env.DEV_WHITELIST ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean),
);

interface MessageGroupAt {
  type: "message_group_at";
  event_id: string;
  group_id: string;
  sender_id: string;
  sender_name: string;
  message_id: string;
  content: string;
}

interface GameState {
  room_id?: string;
  game_id?: string;
}

interface ActionResult {
  message?: string;
  finished?: boolean;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function ensureGroup(redis: Redis) {
  try {
    await redis.xgroup("CREATE", INTERNAL_STREAM, GROUP, "0", "MKSTREAM");
  } catch (err) {
    const msg = String((err as Error)?.message ?? err);
    if (!msg.includes("BUSYGROUP")) throw err;
  }
}

function toMap(fields: string[]): Record<string, string> {
  const map: Record<string, string> = {};
  for (let i = 0; i < fields.length; i += 2) map[fields[i]] = fields[i + 1];
  return map;
}

function parseJson<T>(s: string, fallback: T): T {
  try {
    const v = JSON.parse(s);
    // JSON.parse("null") 返回 null（不抛异常），需显式回退
    return v == null ? fallback : (v as T);
  } catch {
    return fallback;
  }
}

/** 读健康缓存状态（server 后台自检写入）。缓存不存在时返回 null（不降级）。 */
async function readHealth(redis: Redis): Promise<string | null> {
  const s = await redis.get("health:status");
  if (!s) return null;
  try {
    const v = JSON.parse(s);
    return typeof v.status === "string" ? v.status : null;
  } catch {
    return null;
  }
}

function isDevAllowed(senderId: string): boolean {
  return DEV_WHITELIST.has(senderId);
}

async function pushReply(redis: Redis, event: MessageGroupAt, reply: string) {
  const pushEvent = {
    event_id: randomUUID(),
    group_id: event.group_id,
    content: reply,
    reply_msg_id: event.message_id,
    at_member_id: event.sender_id,
  };
  await redis.xadd(PUSH_STREAM, "MAXLEN", "~", "1000", "*", "event", JSON.stringify(pushEvent));
}

async function handle(redis: Redis, engine: GameEngineClient, event: MessageGroupAt) {
  // 健康降级：依赖 unhealthy 时给回执，不现场探测
  const healthStatus = await readHealth(redis);
  if (healthStatus && healthStatus !== "ok") {
    await pushReply(redis, event, "服务繁忙，请稍后重试");
    console.log(`[agent-gateway] 降级回执 health=${healthStatus}`);
    return;
  }

  const { intent, via } = decideIntent(event.content);

  let reply: string;
  try {
    switch (intent) {
      case "chat":
        reply = await chatReply(event.content);
        break;

      case "start_game": {
        // 猜拳：开桌 + 立即出拳（一轮对话）
        const hand = extractHand(event.content);
        if (!hand) {
          reply = "请出「石头/剪刀/布」，例如：来玩猜拳 石头";
          break;
        }
        const stateJson = await engine.startGame("rps", event.group_id, [event.sender_id]);
        const state = parseJson<GameState>(stateJson, {});
        if (!state.room_id) {
          reply = stateJson; // 开桌失败信息
          break;
        }
        const resultJson = await engine.executeAction(state.room_id, event.sender_id, hand);
        const result = parseJson<ActionResult>(resultJson, {});
        reply = result.message ?? resultJson;
        break;
      }

      case "game_action": {
        // 已有活跃对局时出拳
        const hand = extractHand(event.content);
        const activeJson = await engine.getActiveGame(event.group_id);
        const active = parseJson<GameState>(activeJson, {});
        if (active.room_id && hand) {
          const resultJson = await engine.executeAction(active.room_id, event.sender_id, hand);
          const result = parseJson<ActionResult>(resultJson, {});
          reply = result.message ?? resultJson;
        } else if (!hand) {
          reply = "你想出什么？请说「石头/剪刀/布」";
        } else {
          reply = "当前没有进行中的对局，试试「来玩猜拳 石头」";
        }
        break;
      }

      case "dev":
        reply = isDevAllowed(event.sender_id)
          ? "[开发] 已受理（白名单校验通过），开发任务子会话待接入"
          : "[开发] 无权限：你不是开发白名单账号";
        break;

      default:
        reply = await chatReply(event.content);
    }
  } catch (err) {
    console.error(`[agent-gateway] 处理异常 intent=${intent}:`, err);
    reply = "抱歉，处理你的消息时出错了，请稍后再试";
  }

  await pushReply(redis, event, reply);
  console.log(`[agent-gateway] intent=${intent} via=${via} ← ${event.content}`);
}

async function processEntries(redis: Redis, engine: GameEngineClient, results: unknown) {
  for (const [, entries] of results as [string, [string, string[]][]][]) {
    for (const [id, fields] of entries) {
      const map = toMap(fields);
      const json = map["event"];
      if (!json) continue;
      try {
        await handle(redis, engine, JSON.parse(json) as MessageGroupAt);
        await redis.xack(INTERNAL_STREAM, GROUP, id);
      } catch (err) {
        console.error(`[agent-gateway] 处理失败 [${id}]:`, err);
      }
    }
  }
}

async function consume(redis: Redis, engine: GameEngineClient) {
  // 启动时重投 PEL 中未 ACK 的历史消息
  try {
    const pending = (await redis.xreadgroup(
      "GROUP", GROUP, CONSUMER,
      "COUNT", 50,
      "STREAMS", INTERNAL_STREAM, "0",
    )) as unknown;
    if (pending) {
      await processEntries(redis, engine, pending);
      console.log("[agent-gateway] PEL 重投完成");
    }
  } catch (err) {
    console.error("[agent-gateway] 重投 PEL 失败:", err);
  }

  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      const results = (await redis.xreadgroup(
        "GROUP", GROUP, CONSUMER,
        "COUNT", 50,
        "BLOCK", 2000,
        "STREAMS", INTERNAL_STREAM, ">",
      )) as unknown;

      if (results) await processEntries(redis, engine, results);
    } catch (err) {
      const msg = String((err as Error)?.message ?? err);
      // group 被外部删除（如清理脚本）时重建，避免永久 NOGROUP
      if (msg.includes("NOGROUP")) {
        try {
          await ensureGroup(redis);
        } catch {
          /* ignore */
        }
      }
      console.error("[agent-gateway] 消费 internal 失败:", err);
      await sleep(1000);
    }
  }
}

async function main() {
  const redis = new Redis(redisUrl);
  redis.on("error", (e) => console.error("[agent-gateway] redis error:", e.message));

  const engine = new GameEngineClient();
  await engine.connect();

  await ensureGroup(redis);
  console.log(`[agent-gateway] 就绪，消费 ${INTERNAL_STREAM}（dev 白名单 ${DEV_WHITELIST.size} 人）`);
  await consume(redis, engine);
}

main().catch((err) => {
  console.error("[agent-gateway] 致命错误:", err);
  process.exit(1);
});
