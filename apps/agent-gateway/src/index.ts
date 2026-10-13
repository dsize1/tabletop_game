import "dotenv/config";
import { Redis } from "ioredis";
import { randomUUID } from "node:crypto";
import { decideIntent, extractHand } from "./decision.js";
import { SystemOneClient } from "./systemone.js";
import { chatReply, narrateGame, type ChatHistoryTurn } from "./chat.js";
import { GameEngineClient } from "./mcp.js";
import { DevTaskClient } from "./dev-task-client.js";
import { DocsClient } from "./docs-client.js";
import { DevSessionManager, devTaskPrompt } from "./dev-session.js";
import { AuthClient, buildDocsUrl, buildPlanUrl } from "./auth-client.js";
import {
  extractRequirement,
  extractSessionId,
  extractTitle,
  formatCapabilities,
  parseDevCommand,
  splitTaskRef,
  type DevCommand,
} from "./dev-command.js";

const redisUrl = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const INTERNAL_STREAM = "internal";
const PUSH_STREAM = "qq.push";
const GROUP = "agent-gateway";
const CONSUMER = "agent-gateway-1";

// server 内部 HTTP（auth 签发 token）
const SERVER_URL = process.env.SERVER_URL ?? "http://127.0.0.1:3000";
// web 公网基址（cloudflared 域名）；未配置则退回长文推送
const PUBLIC_BASE_URL = (process.env.PUBLIC_BASE_URL ?? "").replace(/\/+$/, "");
const authClient = new AuthClient(SERVER_URL);

// docs-mcp（本地开发文档预览）；在 main() 中 connect
const docsClient = new DocsClient();

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

/** 用给定资源签发短期 token 并拼接短链。失败/未配置返回 null（降级为长文）。 */
async function signPlanLink(sessionId: string, openid: string, resource: string): Promise<string | null> {
  if (!PUBLIC_BASE_URL) return null;
  try {
    const issued = await authClient.issueToken(openid, resource);
    return buildPlanUrl(PUBLIC_BASE_URL, sessionId, issued.token);
  } catch (err) {
    console.error("[agent-gateway] 签发计划短链失败:", err);
    return null;
  }
}

/** 生成计划文档短链（默认资源 dev_task:<sessionId>）。 */
async function buildPlanLink(sessionId: string, openid: string): Promise<string | null> {
  return signPlanLink(sessionId, openid, `dev_task:${sessionId}`);
}

/** #文档：docs-mcp 白名单校验 → 网关签发 docs:all token → 返回文档预览短链。 */
async function handleDocsLink(redis: Redis, event: MessageGroupAt) {
  if (!isDevAllowed(event.sender_id)) {
    await pushReply(redis, event, "[文档] 无权限：你不是开发白名单账号");
    return;
  }
  if (!PUBLIC_BASE_URL) {
    await pushReply(redis, event, "[文档] 未配置 PUBLIC_BASE_URL，无法生成链接");
    return;
  }
  try {
    const json = await docsClient.prepareDocsLink(event.sender_id);
    const info = parseJson<{ resource?: string; roots?: string[] }>(json, {});
    if (!info.resource) {
      await pushReply(redis, event, json);
      return;
    }
    const issued = await authClient.issueToken(event.sender_id, info.resource);
    const url = buildDocsUrl(PUBLIC_BASE_URL, issued.token);
    const roots = (info.roots ?? []).join(" / ");
    await pushReply(redis, event, `开发文档预览（${roots}）：\n${url}`);
  } catch (err) {
    console.error("[agent-gateway] 生成文档链接失败:", err);
    await pushReply(redis, event, "[文档] 生成链接失败，请稍后重试");
  }
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
// ---- dev 子动作（固定前缀命令与自然语言语义共用） ----

async function handleDevList(
  redis: Redis,
  devTask: DevTaskClient,
  event: MessageGroupAt,
  status?: string,
) {
  try {
    const json = await devTask.listTasks(status);
    const tasks = parseJson<DevTaskRow[]>(json, []);
    if (!Array.isArray(tasks) || tasks.length === 0) {
      await pushReply(redis, event, "暂无匹配的开发任务");
      return;
    }
    const lines = tasks.map((t) => `${t.session_id} | ${t.status} | ${t.title ?? "（无标题）"}`);
    await pushReply(redis, event, `开发任务列表（${tasks.length}）：\n` + lines.join("\n"));
  } catch (err) {
    console.error("[agent-gateway] 查询任务失败:", err);
    await pushReply(redis, event, "[开发] 查询任务失败，请稍后重试");
  }
}

async function handleDevQueryById(
  redis: Redis,
  devTask: DevTaskClient,
  event: MessageGroupAt,
  sessionId: string,
) {
  try {
    const json = await devTask.getProgress(sessionId);
    const p = parseJson<DevProgress>(json, {} as DevProgress);
    if (!p.session_id) {
      await pushReply(redis, event, json);
      return;
    }
    const lines = [
      `任务 ${p.session_id}（${p.status}）`,
      `标题：${p.title ?? "（无）"}`,
      `待办 ${p.todo.done}/${p.todo.total} · 验证 ${p.verify.done}/${p.verify.total}`,
    ];
    // token 重签：经 MCP 原语校验归属后由网关签发新短链（原 token 过期后可自助重取）
    if (!["done", "failed", "cancelled"].includes(p.status)) {
      const link = await reissuePlanLink(devTask, sessionId, event.sender_id);
      if (link) lines.push(`计划文档：${link}`);
    }
    await pushReply(redis, event, lines.join("\n"));
  } catch (err) {
    console.error("[agent-gateway] 查询进度失败:", err);
    await pushReply(redis, event, "[开发] 查询进度失败，请稍后重试");
  }
}

async function handleDevConfirmById(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
  sessionId: string,
) {
  try {
    const json = await devTask.confirmTask(sessionId, event.sender_id);
    const task = parseJson<DevTaskRow>(json, {} as DevTaskRow);
    if (task.status === "running") {
      await pushReply(redis, event, `任务 ${sessionId} 已确认，开始执行（异步推进，完成后主动推送）`);
      void runDevResume(
        redis, devTask, devSessions, event, sessionId,
        "用户已确认计划，请解除只读围栏（/plan）并按计划开始执行。",
        "verifying",
      );
    } else if (task.status === "done") {
      await pushReply(redis, event, `任务 ${sessionId} 验收通过，已完成`);
      void runDevResume(
        redis, devTask, devSessions, event, sessionId,
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
}

async function handleDevCancelById(
  redis: Redis,
  devTask: DevTaskClient,
  event: MessageGroupAt,
  sessionId: string,
) {
  try {
    const json = await devTask.cancelTask(sessionId, event.sender_id);
    await pushReply(redis, event, json);
  } catch (err) {
    console.error("[agent-gateway] 取消任务失败:", err);
    await pushReply(redis, event, "[开发] 取消失败，请稍后重试");
  }
}

async function handleDevCapabilities(redis: Redis, devTask: DevTaskClient, event: MessageGroupAt) {
  try {
    const json = await devTask.listCapabilities();
    await pushReply(redis, event, formatCapabilities(json));
  } catch (err) {
    console.error("[agent-gateway] 查询能力失败:", err);
    await pushReply(redis, event, "[开发] 查询能力失败，请稍后重试");
  }
}

/** 固定前缀命令（人 = 决策模型，置信度 100%）。 */
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
    case "任务":
      await handleDevList(redis, devTask, event, cmd.arg);
      break;
    case "进度":
      if (!cmd.arg) { await pushReply(redis, event, "用法：#进度 <sessionId>"); return; }
      await handleDevQueryById(redis, devTask, event, extractSessionId(cmd.arg) ?? cmd.arg);
      break;
    case "确认":
      if (!cmd.arg) { await pushReply(redis, event, "用法：#确认 <sessionId>"); return; }
      await handleDevConfirmById(redis, devTask, devSessions, event, extractSessionId(cmd.arg) ?? cmd.arg);
      break;
    case "取消":
      if (!cmd.arg) { await pushReply(redis, event, "用法：#取消 <sessionId>"); return; }
      await handleDevCancelById(redis, devTask, event, extractSessionId(cmd.arg) ?? cmd.arg);
      break;
    case "能力":
      await handleDevCapabilities(redis, devTask, event);
      break;
    case "发起":
      await handleDevCreate(redis, devTask, devSessions, event, cmd.arg);
      break;
    case "补充":
      if (!cmd.arg) {
        await pushReply(redis, event, "用法：#补充 <sessionId> <正文>");
        return;
      }
      await handleDevAppend(redis, devTask, devSessions, event, cmd.arg);
      break;
  }
}

/** dev 意图但未命中显式命令：只提示，不自动建任务（防止决策引擎误判发起）。 */
async function handleDevUnmatched(redis: Redis, event: MessageGroupAt) {
  if (!isDevAllowed(event.sender_id)) {
    await pushReply(redis, event, "[开发] 无权限：你不是开发白名单账号");
    return;
  }
  await pushReply(
    redis,
    event,
    "检测到可能是开发任务，但需要显式指令（dev 引擎只认命令、不猜意图）：\n" +
      "#发起 <需求> — 创建开发任务\n" +
      "#任务 [状态] — 列出任务\n" +
      "#进度 <sessionId> — 查询任务进度（附新短链）\n" +
      "#确认 <sessionId> — 确认计划\n" +
      "#取消 <sessionId> — 取消任务\n" +
      "#补充 <sessionId> <正文> — 给已有任务追加/修改内容\n" +
      "#文档 — 获取本地开发文档预览链接\n" +
      "#能力 — 查看 MCP 能力清单",
  );
}

/** 追加自由文本到已有任务子会话（#补充）：经 MCP 原语校验后路由到子会话。 */
async function handleDevAppend(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
  arg: string,
) {
  if (!isDevAllowed(event.sender_id)) {
    await pushReply(redis, event, "[开发] 无权限：你不是开发白名单账号");
    return;
  }
  const { sessionId, content } = splitTaskRef(arg);
  if (!sessionId) {
    await pushReply(redis, event, "请提供任务 sessionId，例如：#补充 <id> <正文>");
    return;
  }
  if (!content) {
    await pushReply(redis, event, "请在任务号后附上要追加的正文");
    return;
  }
  try {
    const json = await devTask.appendInstruction(sessionId, event.sender_id);
    const info = parseJson<{ session_id?: string }>(json, {});
    if (!info.session_id) {
      await pushReply(redis, event, json);
      return;
    }
    await pushReply(redis, event, `任务 ${sessionId} 已收到补充，正在追加到执行会话…`);
    void runDevResume(redis, devTask, devSessions, event, sessionId, content, null);
  } catch (err) {
    console.error("[agent-gateway] 追加补充失败:", err);
    await pushReply(redis, event, "[开发] 追加补充失败，请稍后重试");
  }
}

/** 经 dev-task MCP 校验归属+非终态后，由网关签发新的计划短链。 */
async function reissuePlanLink(
  devTask: DevTaskClient,
  sessionId: string,
  openid: string,
): Promise<string | null> {
  try {
    const json = await devTask.prepareTaskLink(sessionId, openid);
    const info = parseJson<{ resource?: string }>(json, {});
    if (!info.resource) return null;
    return await signPlanLink(sessionId, openid, info.resource);
  } catch (err) {
    console.error("[agent-gateway] 重签计划短链失败:", err);
    return null;
  }
}

/** dev 意图：白名单 → create_task → 受理回执（含 sessionId）→ 异步规划。 */
async function handleDevCreate(
  redis: Redis,
  devTask: DevTaskClient,
  devSessions: DevSessionManager,
  event: MessageGroupAt,
  requirementArg?: string,
) {
  if (!isDevAllowed(event.sender_id)) {
    await pushReply(redis, event, "[开发] 无权限：你不是开发白名单账号");
    return;
  }
  const requirement = (requirementArg ?? extractRequirement(event.content)).trim();
  if (!requirement) {
    await pushReply(redis, event, "请提供开发需求，例如：#开发 帮我做一个 XXX 功能");
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
    void runDevCreate(redis, devTask, devSessions, event, sessionId, requirement);
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
      const link = await buildPlanLink(sessionId, event.sender_id);
      const summary = link
        ? `[开发] 计划已生成（任务 ${sessionId}）：\n${extractTitle(output, requirement)}\n计划文档：${link}`
        : `[开发] 计划已生成（任务 ${sessionId}）：\n${output.slice(0, 1500)}`;
      await pushActive(
        redis,
        event,
        `${summary}\n\n回复「#确认 ${sessionId}」开始执行，「#取消 ${sessionId}」放弃`,
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
    if (devCmd.cmd === "文档") {
      await handleDocsLink(redis, event);
      return;
    }
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
        // 仅提示，不自动建任务（dev 只认显式命令，见上方 parseDevCommand）
        await handleDevUnmatched(redis, event);
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

  await docsClient.connect();

  const devSessions = new DevSessionManager();

  await ensureGroup(redis);
  console.log(`[agent-gateway] 就绪，消费 ${INTERNAL_STREAM}（dev 白名单 ${DEV_WHITELIST.size} 人）`);
  await consume(redis, engine, devTask, devSessions);
}

main().catch((err) => {
  console.error("[agent-gateway] 致命错误:", err);
  process.exit(1);
});
