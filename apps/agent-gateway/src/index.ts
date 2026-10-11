import "dotenv/config";
import { Redis } from "ioredis";
import { randomUUID } from "node:crypto";
import { decideIntent, extractHand } from "./decision.js";
import { SystemOneClient } from "./systemone.js";
import { chatReply, narrateGame, type ChatHistoryTurn } from "./chat.js";
import { GameEngineClient } from "./mcp.js";
import { DevTaskClient } from "./dev-task-client.js";
import { DevSessionManager, devTaskPrompt } from "./dev-session.js";
import {
  extractRequirement,
  extractTitle,
  formatCapabilities,
  parseDevCommand,
  type DevCommand,
} from "./dev-command.js";

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

// SystemOne 决策服务（本地 StartLux-Decision，/v1/systemone）
const systemone = new SystemOneClient({
  url: process.env.SYSTEMONE_URL ?? "http://127.0.0.1:8090",
  timeoutMs: Number(process.env.SYSTEMONE_TIMEOUT_MS ?? 3000),
});
// 意图置信度阈值（低于 → #前缀 → 默认 chat）
const INTENT_THRESHOLD = Number(process.env.DECISION_INTENT_THRESHOLD ?? 0.6);

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

interface DevTaskRow {
  session_id: string;
  status: string;
  title?: string | null;
}

interface DevProgress {
  session_id: string;
  status: string;
  title?: string | null;
  todo: { done: number; total: number };
  verify: { done: number; total: number };
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

// 游戏路径 LLM 情景叙述开关：模板摘要必达；开启时追加一条 msg_seq=2 叙述（失败静默降级）
const NARRATION_ENABLED = (process.env.GAME_NARRATION_ENABLED ?? "") === "1";

/** 被动回复（带 reply_msg_id，5 分钟窗口内）。 */
async function pushReply(redis: Redis, event: MessageGroupAt, reply: string, msgSeq = 1) {
  const pushEvent = {
    event_id: randomUUID(),
    group_id: event.group_id,
    content: reply,
    reply_msg_id: event.message_id,
    msg_seq: msgSeq,
    at_member_id: event.sender_id,
  };
  await redis.xadd(PUSH_STREAM, "MAXLEN", "~", "1000", "*", "event", JSON.stringify(pushEvent));
}

/** 主动发送（无 reply_msg_id，异步事件用，受主动消息配额约束）。 */
async function pushActive(redis: Redis, event: MessageGroupAt, content: string) {
  const pushEvent = {
    event_id: randomUUID(),
    group_id: event.group_id,
    content,
    at_member_id: event.sender_id,
  };
  await redis.xadd(PUSH_STREAM, "MAXLEN", "~", "1000", "*", "event", JSON.stringify(pushEvent));
}

// ---- 聊天历史（Redis 热缓存，最近 N 条；全量归档由 bot-adapter 写 SQLite group_messages） ----

const CHAT_HISTORY_LIMIT = Number(process.env.CHAT_HISTORY_LIMIT ?? 20);
const chatHistoryKey = (groupId: string) => `chat:history:${groupId}`;

async function getChatHistory(redis: Redis, groupId: string): Promise<ChatHistoryTurn[]> {
  const raw = await redis.lrange(chatHistoryKey(groupId), -CHAT_HISTORY_LIMIT, -1);
  const out: ChatHistoryTurn[] = [];
  for (const r of raw) {
    const t = parseJson<ChatHistoryTurn>(r, { role: "user", content: r });
    if (t.content) {
      out.push({ role: t.role === "assistant" ? "assistant" : "user", content: t.content });
    }
  }
  return out;
}

async function appendChatHistory(
  redis: Redis,
  groupId: string,
  role: "user" | "assistant",
  content: string,
) {
  await redis.rpush(chatHistoryKey(groupId), JSON.stringify({ role, content }));
  await redis.ltrim(chatHistoryKey(groupId), -CHAT_HISTORY_LIMIT, -1);
}

/** 聊天流程：读历史 → LLM 回复 → 追加历史。 */
async function chatFlow(redis: Redis, event: MessageGroupAt): Promise<string> {
  const history = await getChatHistory(redis, event.group_id);
  const reply = await chatReply(event.content, history);
  await appendChatHistory(redis, event.group_id, "user", event.content);
  await appendChatHistory(redis, event.group_id, "assistant", reply);
  return reply;
}

/** 游戏路径出站：模板摘要必达（msg_seq=1）；开关开启时追加 LLM 情景叙述（msg_seq=2），失败静默降级。 */
async function sendGameReply(
  redis: Redis,
  event: MessageGroupAt,
  template: string,
  actionDesc?: string,
) {
  await pushReply(redis, event, template, 1);
  if (NARRATION_ENABLED && actionDesc) {
    try {
      const narration = await narrateGame(actionDesc, template);
      await pushReply(redis, event, narration, 2);
    } catch (err) {
      console.error("[agent-gateway] 情景叙述失败，静默降级为仅模板:", err);
    }
  }
}

// ==================== dev 任务（固定前缀命令 + 子会话编排） ====================

/** 处理 #任务 / #进度 / #确认 / #取消 / #能力（白名单内）。 */
async function handleDevCommand(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
  cmd: DevCommand,
) {
  if (!isDevAllowed(event.sender_id)) {
    await pushReply(redis, event, "[开发] 无权限：你不是开发白名单账号");
    return;
  }

  switch (cmd.cmd) {
    case "任务": {
      try {
        const json = await devTask.listTasks(cmd.arg);
        const tasks = parseJson<DevTaskRow[]>(json, []);
        if (!Array.isArray(tasks) || tasks.length === 0) {
          await pushReply(redis, event, "暂无匹配的开发任务");
          return;
        }
        const lines = tasks.map(
          (t) => `${t.session_id} | ${t.status} | ${t.title ?? "（无标题）"}`,
        );
        await pushReply(redis, event, `开发任务列表（${tasks.length}）：\n` + lines.join("\n"));
      } catch (err) {
        console.error("[agent-gateway] 查询任务失败:", err);
        await pushReply(redis, event, "[开发] 查询任务失败，请稍后重试");
      }
      break;
    }
    case "进度": {
      if (!cmd.arg) {
        await pushReply(redis, event, "用法：#进度 <sessionId>");
        return;
      }
      try {
        const json = await devTask.getProgress(cmd.arg);
        const p = parseJson<DevProgress>(json, {} as DevProgress);
        if (!p.session_id) {
          await pushReply(redis, event, json);
          return;
        }
        await pushReply(
          redis,
          event,
          `任务 ${p.session_id}（${p.status}）\n标题：${p.title ?? "（无）"}\n待办 ${p.todo.done}/${p.todo.total} · 验证 ${p.verify.done}/${p.verify.total}`,
        );
      } catch (err) {
        console.error("[agent-gateway] 查询进度失败:", err);
        await pushReply(redis, event, "[开发] 查询进度失败，请稍后重试");
      }
      break;
    }
    case "确认": {
      if (!cmd.arg) {
        await pushReply(redis, event, "用法：#确认 <sessionId>");
        return;
      }
      try {
        const json = await devTask.confirmTask(cmd.arg, event.sender_id);
        const task = parseJson<DevTaskRow>(json, {} as DevTaskRow);
        if (task.status === "running") {
          await pushReply(redis, event, `任务 ${cmd.arg} 已确认，开始执行（异步推进，完成后主动推送）`);
          void runDevResume(
            redis, devTask, devSessions, event, cmd.arg,
            "用户已确认计划，请解除只读围栏（/plan）并按计划开始执行。",
            "verifying",
          );
        } else if (task.status === "done") {
          await pushReply(redis, event, `任务 ${cmd.arg} 验收通过，已完成`);
          void runDevResume(
            redis, devTask, devSessions, event, cmd.arg,
            "用户已验收通过，任务完成。请收尾（可选 harness 优化）后终结。",
            null,
          );
        } else {
          await pushReply(redis, event, json);
        }
      } catch (err) {
        console.error("[agent-gateway] 确认任务失败:", err);
        await pushReply(redis, event, "[开发] 确认失败，请稍后重试");
      }
      break;
    }
    case "取消": {
      if (!cmd.arg) {
        await pushReply(redis, event, "用法：#取消 <sessionId>");
        return;
      }
      try {
        const json = await devTask.cancelTask(cmd.arg, event.sender_id);
        await pushReply(redis, event, json);
      } catch (err) {
        console.error("[agent-gateway] 取消任务失败:", err);
        await pushReply(redis, event, "[开发] 取消失败，请稍后重试");
      }
      break;
    }
    case "能力": {
      try {
        const json = await devTask.listCapabilities();
        await pushReply(redis, event, formatCapabilities(json));
      } catch (err) {
        console.error("[agent-gateway] 查询能力失败:", err);
        await pushReply(redis, event, "[开发] 查询能力失败，请稍后重试");
      }
      break;
    }
  }
}

/** dev 意图：白名单 → create_task → 受理回执（含 sessionId）→ 异步规划。 */
async function handleDevCreate(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
) {
  if (!isDevAllowed(event.sender_id)) {
    await pushReply(redis, event, "[开发] 无权限：你不是开发白名单账号");
    return;
  }
  const sessionId = randomUUID();
  try {
    const json = await devTask.createTask(sessionId, event.group_id, event.sender_id);
    const task = parseJson<DevTaskRow>(json, {} as DevTaskRow);
    if (task.status !== "queued") {
      await pushReply(redis, event, json);
      return;
    }
    await devTask.transitionTask(sessionId, "planning");
    await pushReply(redis, event, `[开发] 已受理，任务ID：${sessionId}\n（规划中，完成后主动推送计划）`);
    void runDevCreate(redis, devTask, devSessions, event, sessionId, extractRequirement(event.content));
  } catch (err) {
    console.error("[agent-gateway] 创建开发任务失败:", err);
    await pushReply(redis, event, "[开发] 创建任务失败，请稍后重试");
  }
}

/** 异步：子会话首轮（规划）→ 回填标题 → awaiting_confirm → 主动推送计划。 */
async function runDevCreate(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
  sessionId: string,
  requirement: string,
) {
  await devSessions.withLock(sessionId, async () => {
    let output = "";
    try {
      const { session } = await devSessions.spawn(sessionId);
      try {
        await session.prompt(devTaskPrompt(requirement));
        output = session.getLastAssistantText() ?? "";
      } finally {
        session.dispose();
      }
      await devTask.updateTaskMeta(
        sessionId,
        extractTitle(output, requirement),
        `memory_palace/plan/${sessionId}.md`,
      );
      await devTask.transitionTask(sessionId, "awaiting_confirm");
      await pushActive(
        redis,
        event,
        `[开发] 计划已生成（任务 ${sessionId}）：\n${output.slice(0, 1500)}\n\n回复「#确认 ${sessionId}」开始执行，「#取消 ${sessionId}」放弃`,
      );
    } catch (err) {
      console.error(`[agent-gateway] 开发任务规划失败 sessionId=${sessionId}:`, err);
      await devTask.transitionTask(sessionId, "failed").catch(() => {});
      await pushActive(redis, event, `[开发] 任务 ${sessionId} 规划失败，请稍后重试或「#取消 ${sessionId}」`);
    }
  });
}

/** 异步：恢复子会话跑一轮（确认后执行 / 验收收尾），可选迁移状态。 */
async function runDevResume(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
  sessionId: string,
  message: string,
  afterStatus: string | null,
) {
  await devSessions.withLock(sessionId, async () => {
    try {
      const { session } = await devSessions.resume(sessionId);
      let output = "";
      try {
        await session.prompt(message);
        output = session.getLastAssistantText() ?? "";
      } finally {
        session.dispose();
      }
      if (afterStatus) await devTask.transitionTask(sessionId, afterStatus);
      await pushActive(redis, event, `[开发] 任务 ${sessionId}：\n${output.slice(0, 1500)}`);
    } catch (err) {
      console.error(`[agent-gateway] 开发任务执行失败 sessionId=${sessionId}:`, err);
      await devTask.transitionTask(sessionId, "failed").catch(() => {});
      await pushActive(redis, event, `[开发] 任务 ${sessionId} 执行失败，请查看日志`);
    }
  });
}

// ==================== 主编排 ====================

async function handle(
  redis: Redis,
  engine: GameEngineClient,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
) {
  // 健康降级：依赖 unhealthy 时给回执，不现场探测
  const healthStatus = await readHealth(redis);
  if (healthStatus && healthStatus !== "ok") {
    await pushReply(redis, event, "服务繁忙，请稍后重试");
    console.log(`[agent-gateway] 降级回执 health=${healthStatus}`);
    return;
  }

  // 固定前缀 dev 命令：硬判断，先于决策模型
  const devCmd = parseDevCommand(event.content);
  if (devCmd) {
    await handleDevCommand(redis, devTask, devSessions, event, devCmd);
    return;
  }

  const { intent, via } = await decideIntent(event.content, systemone, {
    threshold: INTENT_THRESHOLD,
  });
  if (via === "unavailable") {
    await pushReply(redis, event, "服务繁忙，请稍后重试");
    console.log("[agent-gateway] 决策服务不可用，降级回执");
    return;
  }

  try {
    switch (intent) {
      case "chat":
        await pushReply(redis, event, await chatFlow(redis, event));
        break;

      case "start_game": {
        // 猜拳：开桌 + 立即出拳（一轮对话）
        const hand = extractHand(event.content);
        if (!hand) {
          await pushReply(redis, event, "请出「石头/剪刀/布」，例如：来玩猜拳 石头");
          break;
        }
        const stateJson = await engine.startGame("rps", event.group_id, [event.sender_id]);
        const state = parseJson<GameState>(stateJson, {});
        if (!state.room_id) {
          await pushReply(redis, event, stateJson); // 开桌失败信息
          break;
        }
        const resultJson = await engine.executeAction(state.room_id, event.sender_id, hand);
        const result = parseJson<ActionResult>(resultJson, {});
        await sendGameReply(redis, event, result.message ?? resultJson, `玩家出了「${hand}」`);
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
          await sendGameReply(redis, event, result.message ?? resultJson, `玩家出了「${hand}」`);
        } else if (!hand) {
          await pushReply(redis, event, "你想出什么？请说「石头/剪刀/布」");
        } else {
          await pushReply(redis, event, "当前没有进行中的对局，试试「来玩猜拳 石头」");
        }
        break;
      }

      case "dev":
        await handleDevCreate(redis, devTask, devSessions, event);
        break;

      default:
        await pushReply(redis, event, await chatFlow(redis, event));
    }
  } catch (err) {
    console.error(`[agent-gateway] 处理异常 intent=${intent}:`, err);
    await pushReply(redis, event, "抱歉，处理你的消息时出错了，请稍后再试");
  }

  console.log(`[agent-gateway] intent=${intent} via=${via} ← ${event.content}`);
}

async function processEntries(
  redis: Redis,
  engine: GameEngineClient,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  results: unknown,
) {
  for (const [, entries] of results as [string, [string, string[]][]][]) {
    for (const [id, fields] of entries) {
      const map = toMap(fields);
      const json = map["event"];
      if (!json) continue;
      try {
        await handle(redis, engine, devTask, devSessions, JSON.parse(json) as MessageGroupAt);
        await redis.xack(INTERNAL_STREAM, GROUP, id);
      } catch (err) {
        console.error(`[agent-gateway] 处理失败 [${id}]:`, err);
      }
    }
  }
}

async function consume(
  redis: Redis,
  engine: GameEngineClient,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
) {
  // 启动时重投 PEL 中未 ACK 的历史消息
  try {
    const pending = (await redis.xreadgroup(
      "GROUP", GROUP, CONSUMER,
      "COUNT", 50,
      "STREAMS", INTERNAL_STREAM, "0",
    )) as unknown;
    if (pending) {
      await processEntries(redis, engine, devTask, devSessions, pending);
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

      if (results) await processEntries(redis, engine, devTask, devSessions, results);
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

  const devTask = new DevTaskClient();
  await devTask.connect();

  const devSessions = new DevSessionManager();

  await ensureGroup(redis);
  console.log(`[agent-gateway] 就绪，消费 ${INTERNAL_STREAM}（dev 白名单 ${DEV_WHITELIST.size} 人）`);
  await consume(redis, engine, devTask, devSessions);
}

main().catch((err) => {
  console.error("[agent-gateway] 致命错误:", err);
  process.exit(1);
});
