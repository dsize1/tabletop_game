import "dotenv/config";
import { Redis } from "ioredis";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  decideIntent,
  decideLinkFetch,
  extractHand,
  extractHttpsLinks,
  parsePrefix,
  DEFAULT_TOPIC_SWITCH_THRESHOLD,
  isTopicSwitch,
  type Intent,
} from "./decision.js";
import {
  clearPending,
  loadPending,
  matchConfirmReply,
  recordAnomaly,
  savePending,
  type PendingIntent,
} from "./intent-confirm.js";
import { SystemOneClient } from "./systemone.js";
import { chatReply, narrateGame, type ChatHistoryTurn } from "./chat.js";
import { emitChatLog } from "./chat-log.js";
import {
  appendChatHistory,
  boundaryDisposition,
  clearPendingBoundary,
  consumePendingBoundary,
  cropToCurrentPhase,
  getChatHistory,
  markPendingBoundary,
} from "./chat-history.js";
import {
  CHAT_TZ,
  chatGapMs,
  formatGap,
  isStaleGap,
  lastUserContent,
} from "./time-context.js";
import { fetchLinkText } from "./link-fetch.js";
import { GameEngineClient } from "./mcp.js";
import { DevTaskClient } from "./dev-task-client.js";
import { DocsClient } from "./docs-client.js";
import { AuthClient, buildDocsUrl, buildGameUrl, buildPlanUrl } from "./auth-client.js";
import { ProfileClient } from "./profile/client.js";
import { ProfileCache } from "./profile/inject.js";
import { LocalLlm, withLocalLlm } from "./profile/local-llm.js";
import { startDailyScheduler } from "./profile/scheduler.js";
import { runDistillJob } from "./profile/distill.js";
import { makeSystemOneGate } from "./profile/gate.js";
import {
  extractRequirement,
  extractSessionId,
  extractTitle,
  formatCapabilities,
  parseDevCommand,
  splitTaskRef,
  type DevCommand,
} from "./dev-command.js";
import {
  parseGameCommand,
  gameActionId,
  lobbyPrompt,
  formatActionList,
  GAME_ACTION_HINT,
  GAME_ACTION_CHOICE_LIMIT,
  type GameCommand,
  type LegalAction,
} from "./game-command.js";
import { directLaunchPrompt, formatGameMenu, resolveGameOption } from "./games.js";

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

// 全局人物画像（跨群，ADR-0007）：经 server /internal/* 读写；带 TTL 内存缓存避免每轮打 server
const profileClient = new ProfileClient({ baseUrl: SERVER_URL });
const PROFILE_CACHE_TTL_MS = Number(process.env.PROFILE_CACHE_TTL_MS ?? 600_000);
const profileCache = new ProfileCache(profileClient, PROFILE_CACHE_TTL_MS);

// 画像日批（ADR-0007 决策 13）：默认关闭；PROFILE_DISTILL_ENABLED=1 开启
const PROFILE_DISTILL_ENABLED = (process.env.PROFILE_DISTILL_ENABLED ?? "0") === "1";
const PROFILE_DISTILL_HOUR = Number(process.env.PROFILE_DISTILL_HOUR ?? 4);
// SystemOne 把关阈值：P(值得写入) ≥ 阈值才保留（经验值，可配）
const PROFILE_GATE_THRESHOLD = Number(process.env.PROFILE_GATE_THRESHOLD ?? 0.5);
// 本地 9B GGUF 路径（固定位置，不迁移；env 必填，缺失则跳过并告警）
const PROFILE_LLM_MODEL = process.env.PROFILE_LLM_MODEL ?? "";

// docs-mcp（本地开发文档预览）；在 main() 中 connect
const docsClient = new DocsClient();

// dev 白名单（member_openid 逗号分隔）
const DEV_WHITELIST = new Set(
  (process.env.DEV_WHITELIST ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean),
);

// dev 执行模式（ADR-0015 B4）：
// - inline（默认）：现状，网关内跑 pi 子会话（B8 前保留回退）
// - entry：只做入口（落库+入队+回执），执行交外部（n8n/LangGraph）
const DEV_EXEC_MODE = (process.env.DEV_EXEC_MODE ?? "inline").trim();
// langgraph = 交 LangGraph 执行器（默认；L3）；entry = 只入口（不执行）
const DEV_EXEC_LANGGRAPH = DEV_EXEC_MODE !== "entry";


// SystemOne 决策服务（本地 StartLux-Decision，/v1/systemone）
const systemone = new SystemOneClient({
  url: process.env.SYSTEMONE_URL ?? "http://127.0.0.1:8090",
  timeoutMs: Number(process.env.SYSTEMONE_TIMEOUT_MS ?? 3000),
});
// 意图置信度双阈值：≥ 高阈值直接路由；[低, 高) 走人工「意图确认」；< 低阈值走 #前缀 → 默认 chat
const INTENT_THRESHOLD = Number(process.env.DECISION_INTENT_THRESHOLD ?? 0.6);
const INTENT_THRESHOLD_HIGH = Number(process.env.DECISION_INTENT_THRESHOLD_HIGH ?? 0.8);
// 未决「意图确认」在 Redis 的存活时长（秒）
const INTENT_CONFIRM_TTL_SEC = Number(process.env.INTENT_CONFIRM_TTL_SEC ?? 300);
// 话题切换：距上次对话 ≥ 该间隔才做「是否新话题」判定（经验值，非外部约束，可配）
const TOPIC_GAP_MINUTES = Number(process.env.TOPIC_GAP_MINUTES ?? 30);
const TOPIC_GAP_MS = Math.max(0, TOPIC_GAP_MINUTES) * 60 * 1000;
// 话题切换阈值：P(true) ≥ 阈值才判新话题（经验值，可配）
const TOPIC_SWITCH_THRESHOLD = Number(
  process.env.TOPIC_SWITCH_THRESHOLD ?? DEFAULT_TOPIC_SWITCH_THRESHOLD,
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
  status?: string;
  data?: {
    phase?: string;
    turn?: number;
    seats?: Array<{ player_id: string; is_bot?: boolean }>;
    small_game_count?: number;
    winners?: string[];
    scores?: Record<string, number>;
    sg?: {
      pending?: {
        actor?: string;
        game?: string;
        stage?: string;
        data?: { winner?: string; total?: number; group?: number };
      };
    };
    roll?: { small?: number[]; big?: number | null };
  };
}

interface ActionResult {
  message?: string;
  finished?: boolean;
  state?: GameState;
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

// 游戏路径 LLM 情景叙述开关（与模板合并为一条回复，失败静默降级）
const NARRATION_ENABLED = (process.env.GAME_NARRATION_ENABLED ?? "") === "1";

// 主动消息默认禁用：开发门禁要求方案含主动消息时先人工确认（见 coding-standards）。
// 仅当显式置为 1 时，被动回复超限才降级为主动消息。
const PROACTIVE_MESSAGES_ENABLED = (process.env.PROACTIVE_MESSAGES_ENABLED ?? "0") === "1";

// 聊天链接抓取（Plan B）：消息含 https:// 时先用 SystemOne（noul 是/否）判断是否抓取该链接
const LINK_FETCH_ENABLED = (process.env.CHAT_LINK_FETCH_ENABLED ?? "1") === "1";
const LINK_FETCH_THRESHOLD = Number(process.env.CHAT_LINK_FETCH_THRESHOLD ?? 0.5);
const LINK_FETCH_TIMEOUT_MS = Number(process.env.CHAT_LINK_TIMEOUT_MS ?? 10000);
const LINK_FETCH_MAX_CHARS = Number(process.env.CHAT_LINK_MAX_CHARS ?? 2000);

/**
 * 被动回复（带 reply_msg_id，5 分钟窗口内）。
 *
 * QQ 限制：同一条消息最多被动回复 **5 次**，且 `msg_seq` 需**递增**。
 * 这里按 `message_id` 用 Redis 计数器递增；超过 5 次自动降级为**主动消息**（不消耗回复次数）。
 */
/** 按字符截断（避免超过 QQ 单条 1500 字符）。 */
function capText(s: string, max: number): string {
  return s.length <= max ? s : s.slice(0, max - 1) + "…";
}

async function pushReply(redis: Redis, event: MessageGroupAt, reply: string, _msgSeq = 1) {
  const key = `msgseq:${event.message_id}`;
  const seq = await redis.incr(key);
  await redis.expire(key, 420);
  if (seq > 5) {
    if (PROACTIVE_MESSAGES_ENABLED) {
      console.warn(
        `[agent-gateway] 被动回复已满 5 次（seq=${seq}），转主动消息（已启用）cid=${event.message_id}`,
      );
      await pushActive(redis, event, reply);
    } else {
      console.warn(
        `[agent-gateway] 被动回复已满 5 次（seq=${seq}），主动消息未启用→丢弃 cid=${event.message_id}`,
      );
    }
    return;
  }
  const pushEvent = {
    event_id: randomUUID(),
    group_id: event.group_id,
    content: reply,
    reply_msg_id: event.message_id,
    msg_seq: seq,
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

// ---- 聊天历史（Redis 热缓存，见 `chat-history.ts`；ioredis 结构上满足 `ListStore`）----

/** 消息含 https:// 链接时：SystemOne 判断是否需要抓取 → 抓取正文追加到上下文（任何失败静默降级）。 */
async function augmentWithLink(content: string): Promise<string> {
  if (!LINK_FETCH_ENABLED) return content;
  const links = extractHttpsLinks(content);
  if (links.length === 0) return content;
  try {
    const d = await decideLinkFetch(content, links, systemone, { threshold: LINK_FETCH_THRESHOLD });
    console.log(
      `[agent-gateway] 链接抓取判断：${d.fetch ? "抓取" : "跳过"}（P=${d.probability.toFixed(2)}）${links[0]}`,
    );
    if (!d.fetch) return content;
    const text = await fetchLinkText(links[0], {
      timeoutMs: LINK_FETCH_TIMEOUT_MS,
      maxChars: LINK_FETCH_MAX_CHARS,
    });
    if (!text) return content;
    console.log(`[agent-gateway] 链接正文已注入（${text.length} 字符）${links[0]}`);
    return `${content}\n\n[链接内容摘录 ${links[0]}]\n${text}`;
  } catch (err) {
    console.error("[agent-gateway] 链接抓取判断失败，按普通聊天处理:", err);
    return content;
  }
}

/** 聊天流程：读历史 →（如需）抓链接正文注入 → LLM 回复 → 追历史（时间只入上下文，不入库）。 */
async function chatFlow(
  redis: Redis,
  event: MessageGroupAt,
  content: string = event.content,
  meta: { boundary?: boolean; history?: ChatHistoryTurn[] } = {},
): Promise<string> {
  // 上游已读过历史则复用（避免同一轮重复 LRANGE）；否则自行读取
  // 阶段裁剪（a1）：只把当前话题阶段（最近 boundary 之后）注入 LLM（ADR-0007 决策 6）
  const history = cropToCurrentPhase(
    meta.history ?? (await getChatHistory(redis, event.group_id, event.sender_id)),
  );
  const userContent = await augmentWithLink(content);
  const profile = await profileCache.get(event.sender_id);
  const reply = await chatReply(userContent, history, { profile });
  await appendChatHistory(redis, event.group_id, event.sender_id, "user", content, meta);
  await appendChatHistory(redis, event.group_id, event.sender_id, "assistant", reply);
  // 冷存归档（chat.log → SQLite chat_history；画像蒸馏数据源）
  await emitChatLog(redis, { openid: event.sender_id, groupId: event.group_id, role: "user", content });
  await emitChatLog(redis, { openid: event.sender_id, groupId: event.group_id, role: "assistant", content: reply });
  return reply;
}

/** 取回对局局面摘要（失败/非文本/错误串 → undefined，不影响模板必达）。 */
async function fetchStateSummary(
  engine: GameEngineClient,
  roomId: string,
): Promise<string | undefined> {
  try {
    const s = await engine.getStateSummary(roomId);
    if (!s || s.startsWith("错误")) return undefined;
    return s;
  } catch (err) {
    console.error("[agent-gateway] 取局面摘要失败，叙述不带局面:", err);
    return undefined;
  }
}

/** 游戏路径出站：模板摘要必达（msg_seq=1）；开关开启时追加 LLM 情景叙述（msg_seq=2，带局面摘要），失败静默降级。 */
async function sendGameReply(
  redis: Redis,
  event: MessageGroupAt,
  template: string,
  actionDesc?: string,
  stateSummary?: string,
) {
  await pushReply(redis, event, template, 1);
  if (NARRATION_ENABLED && actionDesc) {
    try {
      const profile = await profileCache.get(event.sender_id);
      const narration = await narrateGame({
        actor: await resolveActorName(redis, event),
        action: actionDesc,
        result: template,
        stateSummary,
        profileText: profile,
      });
      await pushReply(redis, event, narration, 2);
    } catch (err) {
      console.error("[agent-gateway] 情景叙述失败，静默降级为仅模板:", err);
    }
  }
}

// ==================== 猜拳手势（显式手势 → rps 引擎） ====================

/** 在指定 rps 对局里出拳：执行 → 归档 → 模板回执（+ 可选情景叙述）。 */
async function playRpsHand(
  redis: Redis,
  engine: GameEngineClient,
  event: MessageGroupAt,
  content: string,
  hand: string,
  roomId: string,
): Promise<void> {
  const resultJson = await engine.executeAction(roomId, event.sender_id, hand);
  const result = parseJson<ActionResult>(resultJson, {});
  const summary = await fetchStateSummary(engine, roomId);
  await emitChatLog(redis, { openid: event.sender_id, groupId: event.group_id, role: "user", content });
  await emitChatLog(redis, {
    openid: event.sender_id,
    groupId: event.group_id,
    role: "assistant",
    content: result.message ?? resultJson,
  });
  await sendGameReply(redis, event, result.message ?? resultJson, `玩家出了「${hand}」`, summary);
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
  event: MessageGroupAt,
  sessionId: string,
) {
  try {
    // 审批双语义（ADR-0015 §4）：桥提供 approve_plan / approve_verify（B8 后无 Rust 回退）
    const taskJson = await devTask.getTask(sessionId);
    const cur = parseJson<DevTaskRow>(taskJson, {} as DevTaskRow);
    let json: string;
    if (cur.status === "awaiting_confirm") {
      json = await devTask.approvePlan(sessionId, event.sender_id);
    } else if (cur.status === "verifying") {
      json = await devTask.approveVerify(sessionId, event.sender_id);
    } else {
      await pushReply(redis, event, taskJson);
      return;
    }
    const task = parseJson<DevTaskRow>(json, {} as DevTaskRow);
    if (task.status === "running") {
      await pushReply(redis, event, `任务 ${sessionId} 已确认，开始执行（异步推进，完成后主动推送）`);
      if (DEV_EXEC_LANGGRAPH) {
        runStageViaExecutor(redis, event, sessionId, "execute");
      }
    } else if (task.status === "done") {
      await pushReply(redis, event, `任务 ${sessionId} 验收通过，已完成`);
      if (DEV_EXEC_LANGGRAPH) {
        runStageViaExecutor(redis, event, sessionId, "verify");
      }
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
      await handleDevConfirmById(redis, devTask, event, extractSessionId(cmd.arg) ?? cmd.arg);
      break;
    case "取消":
      if (!cmd.arg) { await pushReply(redis, event, "用法：#取消 <sessionId>"); return; }
      await handleDevCancelById(redis, devTask, event, extractSessionId(cmd.arg) ?? cmd.arg);
      break;
    case "能力":
      await handleDevCapabilities(redis, devTask, event);
      break;
    case "发起":
      await handleDevCreate(redis, devTask, event, cmd.arg);
      break;
    case "补充":
      if (!cmd.arg) {
        await pushReply(redis, event, "用法：#补充 <sessionId> <正文>");
        return;
      }
      await handleDevAppend(redis, devTask, event, cmd.arg);
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
    const text = content;
    if (DEV_EXEC_LANGGRAPH) {
      await pushReply(redis, event, `任务 ${sessionId} 已收到补充，正在追加…`);
      runStageViaExecutor(redis, event, sessionId, "append", text);
    } else {
      await pushReply(redis, event, `任务 ${sessionId} 已收到补充（已记录；当前模式不执行）`);
    }
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
    if (DEV_EXEC_LANGGRAPH) {
      runStageViaExecutor(redis, event, sessionId, "plan", requirement);
    }
  } catch (err) {
    console.error("[agent-gateway] 创建开发任务失败:", err);
    await pushReply(redis, event, "[开发] 创建任务失败，请稍后重试");
  }
}

const PROJECT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");

/** L3：把某个 stage 交给 LangGraph 执行器（`uv run dev-executor work`）。 */
function runStageViaExecutor(
  redis: Redis,
  event: MessageGroupAt,
  sessionId: string,
  stage: "plan" | "execute" | "verify" | "append",
  payload?: string,
) {
  const cwd = path.join(PROJECT_ROOT, "agents", "dev_executor");
  const args = ["run", "dev-executor", "work", "--session-id", sessionId, "--stage", stage];
  if (payload) args.push(stage === "append" ? "--instruction" : "--requirement", payload);
  const child = spawn("uv", args, {
    cwd,
    env: {
      ...process.env,
      SERVER_URL: process.env.SERVER_URL ?? "http://127.0.0.1:3000",
      // 默认 stub（确定性、零成本）；需要真实计划时设 DEV_PLANNER=llm|deepagents
      DEV_PLANNER: process.env.DEV_PLANNER ?? "stub",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  child.stdout.on("data", (b) => console.log(`[dev-exec:${stage}] ${String(b).trim()}`));
  child.stderr.on("data", (b) => console.error(`[dev-exec:${stage}] ${String(b).trim()}`));
  child.on("error", (e) => {
    console.error(`[dev-exec:${stage}] 启动失败 sessionId=${sessionId}:`, e.message);
    void pushActive(redis, event, `[开发] 执行器启动失败（任务 ${sessionId}，stage=${stage}），请稍后重试`);
  });
  child.on("exit", (code) => {
    if (code !== 0) {
      console.error(`[dev-exec:${stage}] 退出码 ${code} sessionId=${sessionId}`);
      void pushActive(redis, event, `[开发] 任务 ${sessionId} 的 ${stage} 阶段失败（exit=${code}），请查看日志`);
    }
  });
}

// ==================== 意图确认（中带置信度的人工二次确认） ====================

const INTENT_LABEL: Record<Intent, string> = {
  chat: "普通聊天",
  start_game: "开启新对局",
  game_action: "游戏动作",
  dev: "开发任务",
};

/** 决策模型原始输出（候选意图:置信度）的日志片段；无模型回答时为 `-`。 */
const fmtModel = (m?: { intent: Intent; confidence: number }): string =>
  m ? `${m.intent}:${m.confidence.toFixed(3)}` : "-";

/** 中带确认提示：明确 y/n。 */
function confirmPrompt(intent: Intent, confidence: number, ttlSec: number): string {
  const label = INTENT_LABEL[intent];
  return (
    `我不太确定你的意图：最可能是「${label}」（置信度 ${confidence.toFixed(2)}）。\n` +
    `回复 y 按「${label}」处理；回复 n 按普通聊天处理。\n` +
    `（${Math.round(ttlSec / 60)} 分钟内有效）`
  );
}

/** 执行意图分支（高置信 / 确认通过 / 兜底共用）；`content` 为实际要处理的消息内容。 */
async function executeIntent(
  redis: Redis,
  engine: GameEngineClient,
  event: MessageGroupAt,
  intent: Intent,
  content: string,
  meta: { boundary?: boolean; history?: ChatHistoryTurn[] } = {},
) {
  switch (intent) {
    case "chat":
      await pushReply(redis, event, await chatFlow(redis, event, content, meta));
      break;

    case "start_game": {
      // 猜拳（显式手势）：优先在当前活跃 rps 局里出拳，否则开一局新的；非手势 → #开局 菜单
      const hand = extractHand(content);
      if (!hand) {
        await handleGameCommand(redis, engine, event, { cmd: "create" });
        break;
      }
      const active = parseJson<GameState>(await engine.getActiveGame(event.group_id), {});
      let roomId = active.game_id === "rps" ? active.room_id : undefined;
      if (!roomId) {
        const stateJson = await engine.startGame("rps", event.group_id, [event.sender_id]);
        const state = parseJson<GameState>(stateJson, {});
        if (!state.room_id) {
          await pushReply(redis, event, stateJson); // 开桌失败信息
          break;
        }
        roomId = state.room_id;
      }
      await playRpsHand(redis, engine, event, content, hand, roomId);
      break;
    }

    case "game_action": {
      // 猜拳活跃对局 + 显式手势：旧行为保留；否则引导用显式游戏命令
      const hand = extractHand(content);
      const activeJson = await engine.getActiveGame(event.group_id);
      const active = parseJson<GameState>(activeJson, {});
      if (active.room_id && hand && active.game_id === "rps") {
        await playRpsHand(redis, engine, event, content, hand, active.room_id);
      } else if (active.room_id) {
        // 阶段 4b：合法动作 → 决策模型选择 → 执行；不可选则回清单兜底
        const legalRaw = await engine.getLegalActions(active.room_id, event.sender_id);
        const legal = parseJson<LegalAction[]>(legalRaw, []);
        if (!Array.isArray(legal) || legal.length === 0) {
          await pushReply(redis, event, "现在还没轮到你操作。" + GAME_ACTION_HINT);
          break;
        }
        const chosen = await chooseGameAction(content, legal);
        if (!chosen) {
          await pushReply(redis, event, formatActionList(legal));
          break;
        }
        const resultJson = await engine.executeAction(active.room_id, event.sender_id, chosen.id);
        if (resultJson.startsWith("错误")) {
          await pushReply(redis, event, resultJson);
          break;
        }
        const result = parseJson<ActionResult>(resultJson, {});
        await emitChatLog(redis, { openid: event.sender_id, groupId: event.group_id, role: "user", content });
        await emitChatLog(redis, {
          openid: event.sender_id,
          groupId: event.group_id,
          role: "assistant",
          content: result.message ?? resultJson,
        });
        const boardUrl = await makeBoardUrl(active.room_id, event.sender_id);
        const names = await loadNames(redis, event.group_id, result.state ?? {});
        const fb = await buildTurnFeedback(
          engine,
          active.room_id,
          result.message ?? resultJson,
          event.sender_id,
          boardUrl,
        );
        await pushGameFeedback(
          redis,
          event,
          fb.text,
          `玩家动作 ${chosen.id}`,
          result.message ?? resultJson,
          fb.summary,
          names,
          actorName(names, event),
        );
        if (!result.finished) {
          await driveBots(redis, engine, event, active.room_id, event.sender_id, boardUrl);
        }
        console.log(
          `[agent-gateway] game_action via=decision model=${chosen.id}:${chosen.confidence.toFixed(3)} ← ${content}`,
        );
      } else {
        await pushReply(redis, event, "当前没有进行中的对局，试试「#开局」选一个桌游。");
      }
      break;
    }

    case "dev":
      // 仅提示，不自动建任务（dev 只认显式命令，见上方 parseDevCommand）
      await handleDevUnmatched(redis, event);
      break;

    default:
      await pushReply(redis, event, await chatFlow(redis, event, content, meta));
  }
}

/** 低阈值兜底：`#前缀` → 默认 chat（对给定消息内容）。 */
async function fallbackLowThreshold(
  redis: Redis,
  engine: GameEngineClient,
  event: MessageGroupAt,
  content: string,
  cid: string,
  via: "confirm_no" | "default" = "default",
  model?: { intent: Intent; confidence: number },
) {
  const prefix = parsePrefix(content);
  const intent: Intent = prefix ?? "chat";
  await executeIntent(redis, engine, event, intent, content);
  console.log(
    `[agent-gateway] intent=${intent} via=${via} fallback=${prefix ? "prefix" : "default"} model=${fmtModel(model)} cid=${cid} ← ${content}`,
  );
}

/** 发起中带确认：同 openid 已有未决确认则覆盖（旧「原消息+确认消息」静默进异常记录）。 */
async function raisePendingConfirm(
  redis: Redis,
  event: MessageGroupAt,
  model: { intent: Intent; confidence: number },
): Promise<void> {
  const existing = await loadPending(redis, event.group_id, event.sender_id);
  if (existing) {
    recordAnomaly("未决确认被同 openid 覆盖", {
      group_id: event.group_id,
      sender_id: event.sender_id,
      prev_cid: existing.cid,
      prev_intent: existing.intent,
      prev_confidence: existing.confidence,
      prev_content: existing.content,
      prev_ts: existing.ts,
      new_cid: event.message_id,
      new_intent: model.intent,
      new_confidence: model.confidence,
      new_content: event.content,
    });
  }
  const pending: PendingIntent = {
    intent: model.intent,
    confidence: model.confidence,
    content: event.content,
    cid: event.message_id,
    reasked: false,
    ts: Date.now(),
  };
  await savePending(redis, event.group_id, event.sender_id, pending, INTENT_CONFIRM_TTL_SEC);
  await pushReply(redis, event, confirmPrompt(model.intent, model.confidence, INTENT_CONFIRM_TTL_SEC));
  console.log(
    `[agent-gateway] intent=${model.intent} via=confirm model=${fmtModel(model)} cid=${event.message_id} ← ${event.content}`,
  );
}

/**
 * 未决「意图确认」处理：
 * - y → 按候选意图执行**原消息**；n → 按**原消息**走低阈值兜底；
 * - 非 y/n 且再次判为中带 → 覆盖（同 openid，记异常）；
 * - 其余非 y/n → 首次追问一次，再次仍非 y/n 判 n（记异常）。
 */
async function handlePendingConfirm(
  redis: Redis,
  engine: GameEngineClient,
  event: MessageGroupAt,
  pending: PendingIntent,
): Promise<void> {
  const verdict = matchConfirmReply(event.content);
  if (verdict === "yes") {
    await clearPending(redis, event.group_id, event.sender_id);
    await executeIntent(redis, engine, event, pending.intent, pending.content);
    // 终止行：确认通过→按原问题执行（cid 关联到原问题，与发起确认那一行成对）
    console.log(
      `[agent-gateway] intent=${pending.intent} via=confirm_yes model=${pending.intent}:${pending.confidence.toFixed(3)} cid=${pending.cid} ← ${pending.content}`,
    );
    return;
  }
  if (verdict === "no") {
    await clearPending(redis, event.group_id, event.sender_id);
    await fallbackLowThreshold(redis, engine, event, pending.content, pending.cid, "confirm_no", {
      intent: pending.intent,
      confidence: pending.confidence,
    });
    return;
  }

  // 非 y/n：若这条本身就是又一次中带请求 → 同 openid 覆盖（点 4）
  const decision = await decideIntent(event.content, systemone, {
    threshold: INTENT_THRESHOLD,
    thresholdHigh: INTENT_THRESHOLD_HIGH,
  });
  if (decision.via === "unavailable") {
    await pushReply(redis, event, "服务繁忙，请稍后重试");
    console.log("[agent-gateway] 决策服务不可用（确认回复），降级回执");
    return;
  }
  if (decision.via === "confirm" && decision.model) {
    await raisePendingConfirm(redis, event, decision.model);
    return;
  }

  if (!pending.reasked) {
    await savePending(
      redis,
      event.group_id,
      event.sender_id,
      { ...pending, reasked: true },
      INTENT_CONFIRM_TTL_SEC,
    );
    await pushReply(redis, event, "未识别，请只回复 y 或 n（仍不回复 y/n 将按 n 处理）。");
    console.log(`[agent-gateway] 意图确认=unknown cid=${pending.cid} → 追问一次 ← ${event.content}`);
    return;
  }

  await clearPending(redis, event.group_id, event.sender_id);
  recordAnomaly("未决确认追问后仍非 y/n，判 n", {
    group_id: event.group_id,
    sender_id: event.sender_id,
    cid: pending.cid,
    pending_intent: pending.intent,
    pending_confidence: pending.confidence,
    pending_content: pending.content,
    reply: event.content,
  });
  console.log(`[agent-gateway] 意图确认=n（追问后仍非 y/n）cid=${pending.cid}`);
  await fallbackLowThreshold(redis, engine, event, pending.content, pending.cid, "confirm_no", {
    intent: pending.intent,
    confidence: pending.confidence,
  });
}

// ==================== 拉斯维加斯游戏命令（显式前缀，确定性路由） ====================

const LAS_VEGAS_GAME_ID = "las-vegas";

/** 签发该玩家的牌面短链（未配置 PUBLIC_BASE_URL 则无）。 */
/** 加密选项页的有效期（秒）：超时后玩家发「#恢复」会重新签发。 */
const SECRET_OPTION_TTL_SECS = 300;

async function makeBoardUrl(
  roomId: string,
  openid: string,
  ttlSecs?: number,
): Promise<string | undefined> {
  if (!PUBLIC_BASE_URL) return undefined;
  try {
    const issued = await authClient.issueToken(openid, `game:${roomId}`, ttlSecs);
    return buildGameUrl(PUBLIC_BASE_URL, LAS_VEGAS_GAME_ID, openid, issued.token);
  } catch (err) {
    console.error("[agent-gateway] 签发游戏 token 失败:", (err as Error)?.message ?? err);
    return undefined;
  }
}

/** 记录发送者昵称（每次 @bot 消息都刷新）。 */
async function recordPlayerName(redis: Redis, event: MessageGroupAt): Promise<void> {
  const name = (event.sender_name ?? "").trim();
  if (!name) return;
  await redis.hset(`names:${event.group_id}`, event.sender_id, name);
}

/** 执行者展示名（群昵称缓存 → 事件昵称 → openid），用于情景叙述的主体。 */
async function resolveActorName(redis: Redis, event: MessageGroupAt): Promise<string> {
  try {
    const cached = await redis.hget(`names:${event.group_id}`, event.sender_id);
    if (cached) return cached;
  } catch {
    /* 缓存不可用则降级 */
  }
  return (event.sender_name ?? "").trim() || event.sender_id;
}

/** 同上，但用已加载的展示名表（`loadNames`），避免重复读缓存。 */
function actorName(names: Record<string, string> | undefined, event: MessageGroupAt): string {
  return (
    (names?.[event.sender_id] ?? "").trim() || (event.sender_name ?? "").trim() || event.sender_id
  );
}

/** 掷骰结果的紧凑文案：小[1,2,3] 大5。 */
function formatRoll(roll?: { small?: number[]; big?: number | null }): string {
  if (!roll) return "";
  const small = roll.small ?? [];
  const hasBig = roll.big !== null && roll.big !== undefined;
  if (small.length === 0 && !hasBig) return "";
  const parts: string[] = [];
  if (small.length) parts.push(`小[${small.join(",")}]`);
  if (hasBig) parts.push(`大${roll.big}`);
  return parts.join(" ");
}

/** 加载展示名：群昵称缓存 + 机器人「机器人N」。 */
async function loadNames(
  redis: Redis,
  groupId: string,
  st: GameState,
): Promise<Record<string, string>> {
  const names: Record<string, string> = {};
  try {
    const h = await redis.hgetall(`names:${groupId}`);
    for (const [k, v] of Object.entries(h ?? {})) if (k && v) names[k] = v;
  } catch {
    /* ignore */
  }
  let botIdx = 0;
  for (const s of st.data?.seats ?? []) {
    if (s.is_bot) {
      botIdx += 1;
      names[s.player_id] = `机器人${botIdx}`;
    }
  }
  return names;
}

/**
 * 把文本中的 openid/机器人 id 替换为展示名（长键优先，避免部分重叠）。
 * **跳过 URL**：牌面/短链里必须保留真实 openid 与 token，不能被昵称污染。
 */
function applyNames(text: string, names: Record<string, string>): string {
  const pairs = Object.entries(names)
    .filter(([id, name]) => id && name)
    .sort((a, b) => b[0].length - a[0].length);
  return text
    .split(/(https?:\/\/\S+)/g)
    .map((part) => {
      if (/^https?:\/\//.test(part)) return part;
      let out = part;
      for (const [id, name] of pairs) out = out.split(id).join(name);
      return out;
    })
    .join("");
}

/** 下一步流程指引：谁在等、该做什么、可用动作（启下）。 */
async function buildGuidance(
  engine: GameEngineClient,
  roomId: string,
  st: GameState,
  openid: string,
): Promise<string> {
  const d = st.data;
  if (st.status === "finished" || d?.phase === "finished" || d?.phase === "aborted") {
    const winners = d?.winners ?? [];
    const scores = d?.scores ?? {};
    const scoreText = Object.entries(scores)
      .map(([p, v]) => `${p.slice(0, 8)}:${Math.round(v / 10000)}万`)
      .join(" ");
    return winners.length
      ? `🏁 本局结束，胜者：${winners.join("、")}${scoreText ? `（${scoreText}）` : ""}`
      : "🏁 本局结束";
  }
  if (!d) return "";
  if (d.phase === "lobby") {
    return "等待房主「#开始」开赛；其他人可「#加入」，房主可「#加机器人」。";
  }
  const actor =
    d.phase === "small_game" ? d.sg?.pending?.actor : d.seats?.[d.turn ?? -1]?.player_id;
  if (!actor) return "";
  const seat = (d.seats ?? []).find((s) => s.player_id === actor);
  if (seat?.is_bot) {
    return `轮到 ${actor} 行动…`;
  }
  const legal = parseJson<LegalAction[]>(await engine.getLegalActions(roomId, actor), []);
  const descs = Array.isArray(legal) ? legal.slice(0, 12).map((a) => a.description) : [];
  const who = actor === openid ? "轮到你" : `轮到 ${actor}`;
  const phaseTag = d.phase === "small_game" ? "处理小游戏" : "下注";
  const rollText = phaseTag === "下注" ? formatRoll(d.roll) : "";
  const rollPrefix = rollText ? `｜掷出 ${rollText}` : "";
  const sg = d.phase === "small_game" ? d.sg?.pending : undefined;
  const sgId = sg?.game;
  const ruleHint = sgId ? `\n📖 规则：#小游戏#${sgId.toLowerCase()}` : "";
  // 两段式/多人分工说明（避免玩家不清楚自己为什么被点名）
  let sgNote = "";
  if (sgId === "G1" && sg?.stage === "split") {
    sgNote = `（你是左手玩家：请把 6 个摸彩标记分成两堆；分完后由赢家 ${sg.data?.winner ?? "?"} 选堆）`;
  } else if (sgId === "G1" && sg?.stage === "pick") {
    sgNote = "（你是赢家：请选一堆领取）";
  } else if (sgId === "F1" && sg?.stage === "group") {
    sgNote = "（第一步：选一组灰骰）";
  } else if (sgId === "F1" && sg?.stage === "casino") {
    sgNote = "（第二步：选放入哪个赌场）";
  }
  return descs.length
    ? `🎯 ${who}${phaseTag}${rollPrefix}，可用动作：\n${sgNote ? sgNote + "\n" : ""}${descs
        .map((x, i) => `${i + 1}) ${x}`)
        .join("\n")}${ruleHint}`
    : `🎯 ${who}${phaseTag}${rollPrefix}${sgNote ? "\n" + sgNote : ""}${ruleHint}`;
}

/** 构造一次动作后的完整反馈：结果 + 局面 + 下一步指引 + 牌面页。 */
async function buildTurnFeedback(
  engine: GameEngineClient,
  roomId: string,
  template: string,
  openid: string,
  boardUrl?: string,
): Promise<{ text: string; summary?: string }> {
  const summary = (await fetchStateSummary(engine, roomId)) ?? undefined;
  const st = parseJson<GameState>(await engine.getState(roomId), {});
  const guidance = await buildGuidance(engine, roomId, st, openid);
  const parts = [template, summary, guidance].filter(
    (x): x is string => typeof x === "string" && x.trim().length > 0,
  );
  if (boardUrl) parts.push(`📄 牌面页：${boardUrl}`);
  return { text: parts.join("\n"), summary };
}

/** 模板必达（msg_seq=1）；开启时追加 LLM 主持人情景叙述（msg_seq=2，失败静默降级）。 */
async function pushGameFeedback(
  redis: Redis,
  event: MessageGroupAt,
  feedback: string,
  actionDesc?: string,
  resultMessage?: string,
  summary?: string,
  names?: Record<string, string>,
  /** 本次动作的执行者展示名（叙述主体）；机器人批量播报传 undefined（动作文案已带名字）。 */
  actor?: string,
) {
  const sub = (s: string) => applyNames(s, names ?? {});
  // 错误 / 中断（行政动作）不做 LLM 情景叙述。
  const skipNarrate =
    (resultMessage ?? feedback).startsWith("错误") ||
    (actionDesc ?? "").includes("lobby:abort");
  let narration: string | undefined;
  if (NARRATION_ENABLED && actionDesc && !skipNarrate) {
    try {
      const profile = await profileCache.get(event.sender_id);
      narration = await narrateGame({
        actor,
        action: sub(actionDesc),
        result: sub(resultMessage ?? feedback),
        stateSummary: summary ? sub(summary) : undefined,
        profileText: profile,
      });
    } catch (err) {
      console.error("[agent-gateway] 情景叙述失败，静默降级:", err);
    }
  }
  // 每个动作合并为**一条**回复（模板 + 叙事），减少被动回复次数。
  const body = sub(feedback);
  const merged = narration ? `${body}\n\n${narration}` : body;
  await pushReply(redis, event, capText(merged, 1400));
}

/**
 * 机器人自动行动：逐个机器人像正常玩家一样播报（结果 + 局面 + 下一步指引 + 牌面页），
 * 直到人类回合或终局。
 */
async function driveBots(
  redis: Redis,
  engine: GameEngineClient,
  event: MessageGroupAt,
  roomId: string,
  openid: string,
  boardUrl?: string,
): Promise<void> {
  const lines: string[] = [];
  const acts: string[] = [];
  for (let i = 0; i < 200; i++) {
    const st = parseJson<GameState>(await engine.getState(roomId), {});
    if (!st.room_id || st.status !== "active" || !st.data) break;
    const d = st.data;
    const actor =
      d.phase === "small_game" ? d.sg?.pending?.actor : d.seats?.[d.turn ?? -1]?.player_id;
    const seat = (d.seats ?? []).find((s) => s.player_id === actor);
    if (!actor || !seat?.is_bot) break;
    const raw = await engine.botAction(roomId, actor);
    if (raw.startsWith("错误")) {
      console.error("[agent-gateway] bot_action 失败:", raw);
      break;
    }
    const result = parseJson<ActionResult>(raw, {});
    // 机器人自己的回合才展示它的掷骰（小游戏交互沿用上一位的 roll）
    const rollText = d.phase === "turn_roll" ? formatRoll(d.roll) : "";
    const prefix = rollText ? ` 掷出 ${rollText}` : "";
    lines.push(`🤖 ${actor}${prefix} → ${result.message ?? ""}`);
    acts.push(`${actor}${prefix} ${result.message ?? ""}`);
  }
  if (lines.length === 0) return;
  // 一回合内所有 AI 动作合并为**一条**消息（结果逐行 + 最终局面 + 下一步指引 + 牌面页）。
  const st = parseJson<GameState>(await engine.getState(roomId), {});
  const names = await loadNames(redis, event.group_id, st);
  const summary = (await fetchStateSummary(engine, roomId)) ?? undefined;
  const guidance = await buildGuidance(engine, roomId, st, openid);
  const parts = [
    lines.join("\n"),
    summary,
    guidance,
    boardUrl ? `📄 牌面页：${boardUrl}` : "",
  ].filter((x): x is string => typeof x === "string" && x.trim().length > 0);
  await pushGameFeedback(
    redis,
    event,
    parts.join("\n"),
    acts.join("；"),
    lines.join("\n"),
    summary,
    names,
  );
}

/**
 * 用决策模型在合法动作里选一个（阶段 4b）：`get_legal_actions` → SystemOne `choice`。
 * 仅当动作数 ≤ `GAME_ACTION_CHOICE_LIMIT` 时单题选择；否则返回 null（调用方走清单兜底）。
 */
async function chooseGameAction(
  content: string,
  legal: LegalAction[],
): Promise<{ id: string; confidence: number } | null> {
  if (legal.length === 0 || legal.length > GAME_ACTION_CHOICE_LIMIT) return null;
  const criteria: Record<string, string> = {};
  for (const a of legal) criteria[a.id] = a.description;
  try {
    const res = await systemone.decide(
      { message: content },
      { action: { type: "choice", instructions: "玩家想执行哪个动作？", criteria } },
    );
    const ans = res.answers?.action;
    if (ans && ans.type === "choice" && legal.some((a) => a.id === ans.choice)) {
      return { id: ans.choice, confidence: ans.confidence };
    }
  } catch (err) {
    console.error("[agent-gateway] 游戏动作决策失败:", (err as Error)?.message ?? err);
  }
  return null;
}

/** 处理显式游戏命令：create/board 之外统一走 MCP execute_action（引擎无条件重校验）。 */
async function handleGameCommand(
  redis: Redis,
  engine: GameEngineClient,
  event: MessageGroupAt,
  cmd: GameCommand,
): Promise<void> {
  const activeJson = await engine.getActiveGame(event.group_id);
  const active = parseJson<GameState>(activeJson, {});
  const roomId = active.room_id;

  if (cmd.cmd === "create") {
    if (roomId) {
      await pushReply(redis, event, "本群已有进行中的对局，先打完或由房主「#中断」再来。");
      return;
    }
    const host = event.sender_name || event.sender_id;
    const game = resolveGameOption(cmd.game);
    if (!game) {
      // 无参 = 列菜单；参数无法识别 = 先报错再列菜单（不落闲聊）
      const hint = cmd.game ? `没有「${cmd.game}」这个桌游。` : "";
      await pushReply(redis, event, [hint, formatGameMenu()].filter(Boolean).join("\n"));
      return;
    }
    if (game.mode === "direct") {
      // 无等待厅：建局即开打（如猜拳）
      const stateJson = await engine.startGame(game.id, event.group_id, [event.sender_id]);
      const st = parseJson<GameState>(stateJson, {});
      if (!st.room_id) {
        await pushReply(redis, event, stateJson);
        return;
      }
      await pushReply(redis, event, directLaunchPrompt(game, host));
      return;
    }
    const stateJson = await engine.createGame(game.id, event.group_id, event.sender_id);
    const st = parseJson<GameState>(stateJson, {});
    if (!st.room_id) {
      await pushReply(redis, event, stateJson);
      return;
    }
    const seats = st.data?.seats?.length ?? 1;
    const n = st.data?.small_game_count ?? 3;
    await pushReply(redis, event, lobbyPrompt(host, seats, n));
    return;
  }

  if (cmd.cmd === "addbot") {
    if (!roomId) {
      await pushReply(redis, event, "本群没有进行中的对局。先发「#开局」（或「#单人」直接开）。");
      return;
    }
    let ok = 0;
    for (let i = 0; i < cmd.n; i++) {
      const raw = await engine.executeAction(roomId, event.sender_id, "lobby:add_bot");
      if (raw.startsWith("错误")) {
        await pushReply(redis, event, raw);
        break;
      }
      ok++;
    }
    if (ok > 0) await pushReply(redis, event, `已加入 ${ok} 个机器人。发「#开始」开赛。`);
    return;
  }

  if (cmd.cmd === "solo") {
    if (roomId) {
      await pushReply(redis, event, "本群已有进行中的对局。先「#中断」或打完。");
      return;
    }
    const stateJson = await engine.createGame(LAS_VEGAS_GAME_ID, event.group_id, event.sender_id);
    const st = parseJson<GameState>(stateJson, {});
    if (!st.room_id) {
      await pushReply(redis, event, stateJson);
      return;
    }
    if (cmd.ids) {
      const raw = await engine.executeAction(
        st.room_id,
        event.sender_id,
        `lobby:set_small_games:${cmd.ids}`,
      );
      if (raw.startsWith("错误")) {
        await pushReply(redis, event, raw);
        return;
      }
    }
    for (let i = 0; i < cmd.n; i++) {
      const raw = await engine.executeAction(st.room_id, event.sender_id, "lobby:add_bot");
      if (raw.startsWith("错误")) break;
    }
    const startJson = await engine.executeAction(st.room_id, event.sender_id, "lobby:start");
    const result = parseJson<ActionResult>(startJson, {});
    const boardUrl = await makeBoardUrl(st.room_id, event.sender_id);
    const names = await loadNames(redis, event.group_id, result.state ?? st);
    const fb = await buildTurnFeedback(
      engine,
      st.room_id,
      `🎲 单人局开始（你 + ${cmd.n} 个机器人）。${result.message ?? ""}`,
      event.sender_id,
      boardUrl,
    );
    await pushGameFeedback(
      redis,
      event,
      fb.text,
      "单人局开始",
      result.message ?? startJson,
      fb.summary,
      names,
      actorName(names, event),
    );
    await driveBots(redis, engine, event, st.room_id, event.sender_id, boardUrl);
    return;
  }

  if (cmd.cmd === "sgrule") {
    await pushReply(redis, event, await engine.getSmallGameRules(cmd.id));
    return;
  }

  if (!roomId) {
    await pushReply(redis, event, "本群没有进行中的对局。发「#开局」（或「#单人」一人开）吧。");
    return;
  }

  if (cmd.cmd === "board") {
    const boardUrl = await makeBoardUrl(roomId, event.sender_id);
    const st = parseJson<GameState>(await engine.getState(roomId), {});
    const names = await loadNames(redis, event.group_id, st);
    const guidance = await buildGuidance(engine, roomId, st, event.sender_id);
    const summary = (await fetchStateSummary(engine, roomId)) ?? "（暂无局面摘要）";
    const parts = [summary, guidance, boardUrl ? `📄 牌面页：${boardUrl}` : ""].filter(Boolean);
    await pushReply(redis, event, applyNames(parts.join("\n"), names));
    return;
  }

  let action: string | null;
  if (cmd.cmd === "pick") {
    // `#选 <N>`：按当前合法动作列表的第 N 项确定性选择（与提示里的编号一致）。
    const legal = parseJson<LegalAction[]>(await engine.getLegalActions(roomId, event.sender_id), []);
    if (!Array.isArray(legal) || legal.length === 0) {
      await pushReply(redis, event, "现在没有可选动作。发「#局面」查看。");
      return;
    }
    if (cmd.n < 1 || cmd.n > legal.length) {
      await pushReply(redis, event, `编号超出范围（当前 1–${legal.length}）。`);
      return;
    }
    action = legal[cmd.n - 1].id;
  } else {
    action = gameActionId(cmd);
  }
  if (!action) {
    await pushReply(redis, event, GAME_ACTION_HINT);
    return;
  }
  const resultJson = await engine.executeAction(roomId, event.sender_id, action);
  if (resultJson.startsWith("错误")) {
    // 校验失败：只回错误本身（不叙事、不附局面，避免误导）
    await pushReply(redis, event, resultJson);
    return;
  }
  const result = parseJson<ActionResult>(resultJson, {});
  await emitChatLog(redis, { openid: event.sender_id, groupId: event.group_id, role: "user", content: event.content });
  await emitChatLog(redis, {
    openid: event.sender_id,
    groupId: event.group_id,
    role: "assistant",
    content: result.message ?? resultJson,
  });
  const needsSecret =
    result.state?.data?.sg?.pending?.game === "A1" &&
    result.state?.data?.sg?.pending?.actor === event.sender_id;
  const boardUrl = await makeBoardUrl(
    roomId,
    event.sender_id,
    needsSecret ? SECRET_OPTION_TTL_SECS : undefined,
  );
  const names = await loadNames(redis, event.group_id, result.state ?? {});
  const fb = await buildTurnFeedback(
    engine,
    roomId,
    result.message ?? resultJson,
    event.sender_id,
    boardUrl,
  );
  await pushGameFeedback(
    redis,
    event,
    fb.text,
    `玩家动作 ${action}`,
    result.message ?? resultJson,
    fb.summary,
    names,
    actorName(names, event),
  );
  if (!result.finished) {
    await driveBots(redis, engine, event, roomId, event.sender_id, boardUrl);
  }
}

// ==================== 主编排 ====================

async function handle(
  redis: Redis,
  engine: GameEngineClient,
  devTask: DevTaskClient,
  event: MessageGroupAt,
) {
  // 记录发送者昵称（用于把 openid 渲染为群昵称）
  await recordPlayerName(redis, event).catch(() => {});

  // 健康降级：依赖 unhealthy 时给回执，不现场探测
  const healthStatus = await readHealth(redis);
  if (healthStatus && healthStatus !== "ok") {
    await pushReply(redis, event, "服务繁忙，请稍后重试");
    console.log(`[agent-gateway] 降级回执 health=${healthStatus}`);
    return;
  }

  // 1) 固定前缀 dev 命令：硬判断（人=100% 置信度），先于一切；命中即清未决确认
  const devCmd = parseDevCommand(event.content);
  if (devCmd) {
    await clearPending(redis, event.group_id, event.sender_id);
    console.log(
      `[agent-gateway] intent=dev via=prefix cmd=${devCmd.cmd} model=- cid=${event.message_id} ← ${event.content}`,
    );
    if (devCmd.cmd === "文档") {
      await handleDocsLink(redis, event);
      return;
    }
    await handleDevCommand(redis, devTask, event, devCmd);
    return;
  }

  // 1.5) 显式游戏命令（拉斯维加斯）：确定性路由（先于决策模型）
  const gameCmd = parseGameCommand(event.content);
  if (gameCmd) {
    await clearPending(redis, event.group_id, event.sender_id);
    console.log(
      `[agent-gateway] intent=game via=prefix cmd=${gameCmd.cmd} model=- cid=${event.message_id} ← ${event.content}`,
    );
    await handleGameCommand(redis, engine, event, gameCmd);
    return;
  }

  // 2) 时间上下文（仅记录 + 判据，不注入此路径）：距上次对话间隔驱动「话题切换」与 pending 失效
  //    会话键按 (群, 用户) 隔离（ADR-0007 决策 3）；gap/lastUserContent 基于未裁剪的完整窗口
  const history = await getChatHistory(redis, event.group_id, event.sender_id);
  const gapMs = chatGapMs(history);
  // 门控可整体关闭（`TOPIC_GAP_MINUTES=0`），此时不做话题判定也不使 pending 失效
  const staleGap = isStaleGap(gapMs, TOPIC_GAP_MS);
  const askTopicSwitch = staleGap;

  // 3) 未决「意图确认」：本轮先交确认流程（y/n 直接消费；非 y/n 按规则追问/覆盖）
  const pending = await loadPending(redis, event.group_id, event.sender_id);
  if (pending) {
    // C：跨越 ≥ TOPIC_GAP_MINUTES 的 pending 已失去语境（确认提示的有效期 = 该间隔），确定性丢弃。
    if (staleGap) {
      await clearPending(redis, event.group_id, event.sender_id);
      console.log(
        `[agent-gateway] pending 失效丢弃 gap=${formatGap(gapMs!)} >= ${TOPIC_GAP_MINUTES}min cid=${pending.cid} intent=${pending.intent}`,
      );
    } else {
      await handlePendingConfirm(redis, engine, event, pending);
      return;
    }
  }

  // 4) 决策模型三档：≥ 高阈值直接路由；中带发起确认；低阈值走前缀/默认兜底
  //    `askTopicSwitch` 时额外一问，但仍在**同一次** HTTP 请求内（不增加往返延迟）
  const { intent, via, model, topicSwitch } = await decideIntent(event.content, systemone, {
    threshold: INTENT_THRESHOLD,
    thresholdHigh: INTENT_THRESHOLD_HIGH,
    askTopicSwitch,
    previousMessage: lastUserContent(history),
  });
  if (via === "unavailable") {
    await pushReply(redis, event, "服务繁忙，请稍后重试");
    console.log("[agent-gateway] 决策服务不可用，降级回执");
    return;
  }
  if (via === "confirm" && model) {
    await raisePendingConfirm(redis, event, model);
    return;
  }

  // A：判定为新话题 → 在**本轮** history 条目上打边界标记（决策一次、长期复用）
  //    门控命中时以本次判定为准（丢弃旧暂存）；未命中时取用之前暂存的边界
  let boundary = askTopicSwitch && isTopicSwitch(topicSwitch, TOPIC_SWITCH_THRESHOLD);
  if (askTopicSwitch) {
    await clearPendingBoundary(redis, event.group_id, event.sender_id);
    console.log(
      `[agent-gateway] 话题切换判定 gap=${formatGap(gapMs!)} P=${topicSwitch?.toFixed(2) ?? "-"} → ${boundary ? "新话题" : "延续"}`,
    );
  } else if (await consumePendingBoundary(redis, event.group_id, event.sender_id)) {
    boundary = true;
    console.log("[agent-gateway] 话题切换：取用待落位边界（上一条非 chat 消息上检出）");
  }

  // 边界去向：chat 分支直接落 history；其它分支不写 history → 暂存等下一条 chat 消息补落
  const where = boundaryDisposition(boundary, intent);
  if (where === "stash") {
    await markPendingBoundary(redis, event.group_id, event.sender_id);
    console.log(`[agent-gateway] 话题边界暂存（intent=${intent} 不写 history）`);
  }

  try {
    await executeIntent(redis, engine, event, intent, event.content, {
      boundary: where === "history",
      history,
    });
  } catch (err) {
    console.error(`[agent-gateway] 处理异常 intent=${intent}:`, err);
    await pushReply(redis, event, "抱歉，处理你的消息时出错了，请稍后再试");
  }

  console.log(
    `[agent-gateway] intent=${intent} via=${via} model=${fmtModel(model)} cid=${event.message_id} ← ${event.content}`,
  );
}

async function processEntries(
  redis: Redis,
  engine: GameEngineClient,
  devTask: DevTaskClient,
  results: unknown,
) {
  for (const [, entries] of results as [string, [string, string[]][]][]) {
    for (const [id, fields] of entries) {
      const map = toMap(fields);
      const json = map["event"];
      if (!json) continue;
      try {
        await handle(redis, engine, devTask, JSON.parse(json) as MessageGroupAt);
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
) {
  // 启动时重投 PEL 中未 ACK 的历史消息
  try {
    const pending = (await redis.xreadgroup(
      "GROUP", GROUP, CONSUMER,
      "COUNT", 50,
      "STREAMS", INTERNAL_STREAM, "0",
    )) as unknown;
    if (pending) {
      await processEntries(redis, engine, devTask, pending);
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

      if (results) await processEntries(redis, engine, devTask, results);
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

// ==================== dev 事件 → 出站通知（B7；passive/主动口径见 ADR-0009） ====================

const DEV_NOTIFY_INTERVAL_MS = Number(process.env.DEV_NOTIFY_INTERVAL_MS ?? 5000);

/** 轮询 `dev_task_events`，把 plan_ready / executed / run_summary 推给发起人。
 *  只处理启动后产生的事件（避免历史任务积压刷屏）；游标存 Redis。
 */
function startDevEventNotifier(redis: Redis, devTask: DevTaskClient) {
  if (process.env.DEV_NOTIFY_ENABLED === "0") {
    console.log("[agent-gateway] dev 事件通知已关闭（DEV_NOTIFY_ENABLED=0）");
    return;
  }
  const startedAt = Math.floor(Date.now() / 1000);
  setInterval(() => {
    void (async () => {
      try {
        const tasks = parseJson<Array<Record<string, any>>>(await devTask.listTasks(), []);
        for (const t of tasks) {
          const sid = String(t.session_id ?? "");
          const gid = String(t.group_id ?? "");
          const qq = String(t.qq_id ?? "");
          if (!sid || !gid || !qq) continue;
          const key = `dev:notify:cursor:${sid}`;
          const since = Number((await redis.get(key)) ?? 0);
          const events = parseJson<Array<Record<string, any>>>(
            await devTask.listEvents(sid, since, 50),
            [],
          );
          if (!events.length) continue;
          let maxId = since;
          const fake = { group_id: gid, sender_id: qq } as MessageGroupAt;
          for (const e of events) {
            maxId = Math.max(maxId, Number(e.id ?? 0));
            // 启动前的积压跳过；但 langgraph 任务的补发例外（游标保证只发一次）
            if (Number(e.created_at ?? 0) < startedAt && t.executor !== "langgraph") continue;
            if (e.kind === "plan_ready") {
              const link = await buildPlanLink(sid, qq);
              await pushActive(
                redis,
                fake,
                `[开发] 计划已生成（任务 ${sid}）：${t.title ?? ""}
${link ? `计划文档：${link}` : ""}
回复「#确认 ${sid}」开始执行`,
              );
            } else if (e.kind === "executed") {
              await pushActive(redis, fake, `[开发] 任务 ${sid} 执行完成，待验收。回复「#确认 ${sid}」验收`);
            } else if (e.kind === "notify_requested") {
              let text = "";
              try {
                text = String(JSON.parse(e.payload ?? "{}").text ?? "");
              } catch {
                text = "";
              }
              if (text.trim()) await pushActive(redis, fake, text);
            } else if (e.kind === "run_summary") {
              await pushActive(redis, fake, `[开发] 任务 ${sid} 已完成 ✅`);
            }
          }
          await redis.set(key, String(maxId));
        }
      } catch (err) {
        console.error("[agent-gateway] dev 事件通知失败:", err);
      }
    })();
  }, DEV_NOTIFY_INTERVAL_MS);
  console.log(`[agent-gateway] dev 事件通知已启动（interval=${DEV_NOTIFY_INTERVAL_MS}ms）`);
}

// ==================== 人物画像日批（ADR-0007） ====================

/** 把关（SystemOne）：候选事实是否写入长期画像；失败降级为保留（不因把关故障丢信息）。 */
const profileGate = makeSystemOneGate(systemone, PROFILE_GATE_THRESHOLD);

/** 启动画像日批调度（默认关闭；PROFILE_DISTILL_ENABLED=1 开启）。 */
function startProfileScheduler(redis: Redis): void {
  if (!PROFILE_DISTILL_ENABLED) {
    console.log("[agent-gateway] 画像日批未启用（PROFILE_DISTILL_ENABLED=0）");
    return;
  }
  if (!PROFILE_LLM_MODEL) {
    console.error(
      "[agent-gateway] 画像日批已启用但未配置 PROFILE_LLM_MODEL（本地 9B GGUF 路径），跳过",
    );
    return;
  }
  const llm = new LocalLlm({
    modelPath: PROFILE_LLM_MODEL,
    llamaBin: process.env.PROFILE_LLM_BIN ?? "llama-server",
    port: Number(process.env.PROFILE_LLM_PORT ?? 8082),
    ngl: process.env.PROFILE_LLM_NGL ?? "99",
    ctx: process.env.PROFILE_LLM_CTX ?? "8192",
  });
  const stop = startDailyScheduler({
    hour: PROFILE_DISTILL_HOUR,
    run: async () => {
      await withLocalLlm(llm, async (l) => {
        const res = await runDistillJob({
          client: profileClient,
          generate: (msgs) => l.generate(msgs),
          gate: profileGate,
          kv: redis,
          onProgress: (m) => console.log(`[profile] ${m}`),
        });
        profileCache.invalidateAll();
        console.log(
          `[profile] 蒸馏完成：活跃 ${res.activeOpenids}，更新 ${res.updated}，跳过 ${res.skipped}`,
        );
      });
    },
  });
  console.log(
    `[agent-gateway] 画像日批已启用（每日 ${PROFILE_DISTILL_HOUR}:00 ${CHAT_TZ}；模型 ${PROFILE_LLM_MODEL}）`,
  );
  process.once("SIGINT", () => {
    stop();
    process.exit(0);
  });
}

async function main() {
  const redis = new Redis(redisUrl);
  redis.on("error", (e) => console.error("[agent-gateway] redis error:", e.message));

  const engine = new GameEngineClient();
  await engine.connect();

  const devTask = new DevTaskClient();
  await devTask.connect();

  await docsClient.connect();

  startProfileScheduler(redis);
  startDevEventNotifier(redis, devTask);

  await ensureGroup(redis);
  console.log(`[agent-gateway] 就绪，消费 ${INTERNAL_STREAM}（dev 白名单 ${DEV_WHITELIST.size} 人）`);
  await consume(redis, engine, devTask);
}

main().catch((err) => {
  console.error("[agent-gateway] 致命错误:", err);
  process.exit(1);
});
