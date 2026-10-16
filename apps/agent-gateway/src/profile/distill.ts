//! 画像蒸馏管道（ADR-0007 决策 13）：**生成归本地 9B、把关归 SystemOne**。
//!
//! - 增量：`distilled_at` 游标（Redis）→ 窗口 `[last, now)`；任务失败重跑不漏窗。
//! - **素材只用「用户轮」发言**（机器人回复一律不喂给生成模型）——避免把机器人内容错当用户画像。
//! - 生成：9B 产出候选事实（每行一条）；空输出重试一次。长历史按字符**分块 map-reduce**。
//! - 把关：SystemOne `noul` 逐条判定「是否写入长期画像」（临时/敏感/无信息量则拒，称呼/昵称保留）。
//! - 组装：确定性拼纯文本（按条、限长）；无内容则跳过写回。
//!
//! 生成器与把关器均可注入，便于离线单测。

import type { ChatMessageRow, ProfileClient } from "./client.js";
import type { LlmMessage } from "./local-llm.js";

/** 画像文本上限（字符）。 */
export const PROFILE_MAX_CHARS = Number(process.env.PROFILE_MAX_CHARS ?? 1200);
/** 单块送入 9B 的字符上限（长历史分块）。 */
export const PROFILE_CHUNK_CHARS = Number(process.env.PROFILE_CHUNK_CHARS ?? 6000);
/** 低于该用户发言条数则不做画像（信息太少，易产出噪声）。 */
export const PROFILE_MIN_USER_MESSAGES = Number(process.env.PROFILE_MIN_USER_MESSAGES ?? 3);

/** 增量游标键（Redis）。 */
export const DISTILL_CURSOR_KEY = "profile:distill:cursor";

/** 最小 KV 接口（ioredis 兼容；测试可注入内存实现）。 */
export interface KvStore {
  get(key: string): Promise<string | null>;
  set(key: string, value: string, mode: "EX", ttl: number): Promise<unknown>;
}

/** 蒸馏系统提示（抽取器 + few-shot；对 3B 级小模型友好）。素材只有用户发言。 */
export const DISTILL_SYSTEM_PROMPT = [
  "你是一个信息抽取器，从群聊记录里提炼「某个人」的长期画像要点。",
  "",
  "规则：",
  "- 只提炼稳定、关于本人的信息：称呼/昵称/外号、口头禅、兴趣爱好、职业或身份线索、稳定性格。",
  "- 不要提炼：话题内容、题目、一次性事件、临时情绪、技术/业务细节。",
  "- 每条一行，直接写要点；不要复述原句、不要编号、不要解释。",
  "- 没有可提炼内容时，只输出：无",
  "",
  "示例输入：",
  "用户：大家都叫我老张",
  "用户：我平时就爱打德州扑克",
  "用户：帮我查下明天天气",
  "示例输出：",
  "- 被称作「老张」",
  "- 喜欢打德州扑克",
].join("\n");

export function buildDistillMessages(oldProfile: string, userUtterances: string): LlmMessage[] {
  return [
    { role: "system", content: DISTILL_SYSTEM_PROMPT },
    {
      role: "user",
      content: `【已有画像】\n${oldProfile || "（暂无）"}\n\n【用户发言】\n${userUtterances}\n\n输出：`,
    },
  ];
}

/** 聊天模板控制符（如 `<|im_start|>`/`<|im_end|>`）：用户误发模型自身提示/模板时会带。 */
const CTRL_TOKEN_RE = /<\|[^|]*\|>/;
/** 单条用户发言字符上限（超长多为粘贴垃圾，丢弃）。 */
const MAX_UTTERANCE_CHARS = 1000;

/**
 * 某用户的**用户轮**发言文本。
 * - 去掉 assistant 轮；
 * - **整条丢弃**含聊天模板控制符的发言（常为误发模型提示，会带偏 9B 输出）；
 * - 整条丢弃超长发言；折叠空白。
 */
export function userUtterances(rows: readonly ChatMessageRow[]): string[] {
  return rows
    .filter((r) => r.role !== "assistant")
    .map((r) => r.content.replace(/\s+/g, " ").trim())
    .filter(
      (s) => s.length > 0 && s.length <= MAX_UTTERANCE_CHARS && !CTRL_TOKEN_RE.test(s),
    );
}

/** 渲染对话窗口为文本（用户 / 机器人；调试用途）。 */
export function renderDialogue(rows: readonly ChatMessageRow[]): string {
  return rows
    .map((r) => `${r.role === "assistant" ? "机器人" : "用户"}：${r.content}`)
    .join("\n");
}

/** 按字符分块（保留整条边界）。 */
export function chunkLines(
  lines: readonly string[],
  maxChars = PROFILE_CHUNK_CHARS,
): string[] {
  const chunks: string[] = [];
  let cur: string[] = [];
  let len = 0;
  for (const ln of lines) {
    if (cur.length > 0 && len + ln.length + 1 > maxChars) {
      chunks.push(cur.join("\n"));
      cur = [];
      len = 0;
    }
    cur.push(ln);
    len += ln.length + 1;
  }
  if (cur.length) chunks.push(cur.join("\n"));
  return chunks;
}

/** 解析 9B 输出的候选事实（去项目符号/序号、去空行、去重保序）。 */
export function parseCandidateFacts(raw: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const line of raw.split(/\r?\n/)) {
    let s = line.trim();
    if (!s) continue;
    s = s
      .replace(/^[-*•]\s*/, "")
      .replace(/^\d+[.、)]\s*/, "")
      .trim();
    if (!s || seen.has(s) || s === "无" || s === "（无）") continue;
    seen.add(s);
    out.push(s);
  }
  return out;
}

/** 组装纯文本画像（每行一条，超长截断）。 */
export function assembleProfile(facts: readonly string[], maxChars = PROFILE_MAX_CHARS): string {
  const text = facts.map((f) => `- ${f}`).join("\n");
  return text.length <= maxChars ? text : text.slice(0, maxChars);
}

export interface DistillDeps {
  client: Pick<
    ProfileClient,
    "fetchActiveOpenids" | "fetchChatWindow" | "getProfile" | "upsertProfile"
  >;
  /** 生成（本地 9B） */
  generate: (messages: LlmMessage[]) => Promise<string>;
  /** 把关（SystemOne）：该候选事实是否写入长期画像 */
  gate: (candidate: string, oldProfile: string) => Promise<boolean>;
  now?: () => number;
  maxChars?: number;
  chunkChars?: number;
  /** 低于该用户发言条数则跳过（默认 `PROFILE_MIN_USER_MESSAGES`） */
  minUsers?: number;
  onProgress?: (msg: string) => void;
}

export interface DistillResult {
  activeOpenids: number;
  updated: number;
  skipped: number;
}

/** 对窗口 `[from, to)` 内的活跃用户各做一次蒸馏（分块 map-reduce）。 */
export async function distillWindow(
  from: string,
  to: string,
  deps: DistillDeps,
): Promise<DistillResult> {
  const now = deps.now ?? Date.now;
  const ids = await deps.client.fetchActiveOpenids(from, to);
  const rows = await deps.client.fetchChatWindow(from, to);
  const byUser = new Map<string, ChatMessageRow[]>();
  for (const r of rows) {
    const arr = byUser.get(r.openid);
    if (arr) arr.push(r);
    else byUser.set(r.openid, [r]);
  }
  const minUsers = deps.minUsers ?? PROFILE_MIN_USER_MESSAGES;
  let updated = 0;
  let skipped = 0;
  for (const openid of ids) {
    const lines = userUtterances(byUser.get(openid) ?? []);
    if (lines.length < minUsers) {
      skipped++;
      deps.onProgress?.(`发言 ${lines.length} 条 < ${minUsers}，跳过 openid=${openid}`);
      continue;
    }
    const old = await deps.client.getProfile(openid);
    const oldText = old?.profile_text ?? "";
    const chunks = chunkLines(lines, deps.chunkChars);
    const candidates = new Set<string>();
    for (const ch of chunks) {
      let raw = await deps.generate(buildDistillMessages(oldText, ch));
      if (!raw.trim()) raw = await deps.generate(buildDistillMessages(oldText, ch)); // 空输出重试一次
      for (const f of parseCandidateFacts(raw)) candidates.add(f);
    }
    const kept: string[] = [];
    for (const c of candidates) {
      if (await deps.gate(c, oldText)) kept.push(c);
    }
    const text = assembleProfile(kept, deps.maxChars);
    if (!text) {
      skipped++;
      deps.onProgress?.(`画像为空，跳过写回 openid=${openid}`);
      continue;
    }
    await deps.client.upsertProfile(openid, text, to, new Date(now()).toISOString());
    updated++;
    deps.onProgress?.(`画像已更新 openid=${openid}（保留 ${kept.length}/${candidates.size} 条）`);
  }
  return { activeOpenids: ids.length, updated, skipped };
}

/** 读游标 → 跑蒸馏 → 成功后推进游标（默认窗口回退 24h）。 */
export async function runDistillJob(
  deps: DistillDeps & { kv: KvStore; windowMs?: number; cursorTtlSec?: number },
): Promise<DistillResult> {
  const nowMs = (deps.now ?? Date.now)();
  const to = new Date(nowMs).toISOString();
  const last = await deps.kv.get(DISTILL_CURSOR_KEY);
  const from = last ?? new Date(nowMs - (deps.windowMs ?? 24 * 3600_000)).toISOString();
  const res = await distillWindow(from, to, deps);
  await deps.kv.set(DISTILL_CURSOR_KEY, to, "EX", deps.cursorTtlSec ?? 7 * 24 * 3600);
  return res;
}
