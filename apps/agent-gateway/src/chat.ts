//! 聊天分支：接入 DeepSeek LLM（OpenAI 兼容接口）。
//! 回复限长 1500 字符：超长时模型多做一步「摘要压缩」，保证回复不超限。
//! 时间上下文（当前时间 + 历史时间锚点）只注入 LLM，不落库、不出站，见 `time-context.ts`。

import {
  formatTimeBlock,
  lastTurnTs,
  renderHistory,
  type TimedTurn,
} from "./time-context.js";

const LLM_BASE_URL = process.env.LLM_BASE_URL ?? "https://api.deepseek.com";
const LLM_API_KEY = process.env.LLM_API_KEY ?? "";
const LLM_MODEL = process.env.LLM_MODEL ?? "deepseek-flash";
const LLM_TIMEOUT_MS = Number(process.env.LLM_TIMEOUT_MS ?? 30000);
const LLM_MAX_TOKENS = Number(process.env.LLM_MAX_TOKENS ?? 2000);
// 思考模式默认开启：被动回复窗口实为 5 分钟（见 docs/conventions/external-constraints.md），
// 之前的「3s 超时」系误判，不再为省延迟默认关闭。LLM_THINKING_ENABLED=0 可显式关闭。
const LLM_THINKING_ENABLED = (process.env.LLM_THINKING_ENABLED ?? "1") === "1";

/** 回复最大字符数（QQ 消息有长度限制，超限会报 40054007）。 */
const MAX_REPLY_CHARS = 1500;

// 时间只作为上下文私有层：模型据此判断对话间隔，但不得主动报时（用户没问就不提）。
const SYSTEM_PROMPT =
  "你是桌游群里的助手机器人，回复简洁、友好、口语化，用中文。" +
  "除非用户主动问，不要在回复里提时间（不要说现在几点、今天几号）；" +
  "上下文里出现的时间信息只用于你自己判断对话隔了多久。";

const SUMMARY_PROMPT =
  "请把下面这段内容压缩成一段不超过 1500 字符的回复，保留核心信息和语气，直接输出压缩后的文本，不要任何解释或前缀。";

/** 回显桩兜底：LLM 不可用时的降级回复。 */
function fallback(content: string): string {
  return `[聊天桩] 收到：${content}`;
}

/** 底层 LLM 调用（带超时），返回正文（过滤 reasoning_content）。 */
async function callLlm(
  messages: Array<{ role: string; content: string }>,
  maxTokens: number,
): Promise<string> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), LLM_TIMEOUT_MS);
  try {
    const resp = await fetch(`${LLM_BASE_URL}/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${LLM_API_KEY}`,
      },
      body: JSON.stringify({
        model: LLM_MODEL,
        messages,
        max_tokens: maxTokens,
        // 开启时不传 thinking（由模型默认行为决定）；关闭时显式 disabled（快、省 token）
        ...(LLM_THINKING_ENABLED ? {} : { thinking: { type: "disabled" } }),
        stream: false,
      }),
      signal: controller.signal,
    });

    if (!resp.ok) {
      const body = await resp.text().catch(() => "");
      throw new Error(`LLM HTTP ${resp.status}: ${body.slice(0, 200)}`);
    }

    const data = (await resp.json()) as {
      choices?: Array<{ message?: { content?: string } }>;
    };
    return data.choices?.[0]?.message?.content?.trim() ?? "";
  } finally {
    clearTimeout(timer);
  }
}

/** 聊天历史单条（用于注入 LLM 上下文）：带 `ts` / `boundary` 以支持时间锚点与话题边界。 */
export type ChatHistoryTurn = TimedTurn;

export interface ChatMessagesOptions {
  /** 当前时刻（epoch ms），便于测试注入假时钟。 */
  now?: number;
  /** 时区覆盖（默认 `time-context.CHAT_TZ`）。 */
  tz?: string;
}

/**
 * 组装送 LLM 的 messages（纯函数，便于单测）：
 * `system` 人设 → 历史（簇首带时间锚点、边界带话题切换标记）→ `system` 时间块 → 当前用户消息。
 *
 * 注意「时间只入上下文」：时间块是独立条目，用户消息与历史 `content` 均原样不改写。
 */
export function buildChatMessages(
  content: string,
  history: readonly ChatHistoryTurn[] = [],
  opts: ChatMessagesOptions = {},
): Array<{ role: string; content: string }> {
  const now = opts.now ?? Date.now();
  return [
    { role: "system", content: SYSTEM_PROMPT },
    ...renderHistory(history, { tz: opts.tz }),
    { role: "system", content: formatTimeBlock(now, lastTurnTs(history), opts.tz) },
    { role: "user", content },
  ];
}

/** 聊天回复：正常 LLM 回复（带群历史 + 时间上下文）；超 1500 字符时再做一步摘要压缩。 */
export async function chatReply(
  content: string,
  history: ChatHistoryTurn[] = [],
  opts: ChatMessagesOptions = {},
): Promise<string> {
  if (!LLM_API_KEY) {
    console.error("[chat] 回退桩：未配置 LLM_API_KEY");
    return fallback(content);
  }

  try {
    const messages = buildChatMessages(content, history, opts);
    // 第一段：正常回复
    const text = await callLlm(messages, LLM_MAX_TOKENS);
    if (!text) {
      // 静默回退的根因多在此：思考模式开启时正文可能被 reasoning 占满而返回空。
      console.error(
        `[chat] 回退桩：LLM 返回空正文（model=${LLM_MODEL} thinking=${LLM_THINKING_ENABLED ? "on" : "off"}）`,
      );
      return fallback(content);
    }
    if (text.length <= MAX_REPLY_CHARS) return text;

    // 第二段：超长 → 摘要压缩到 1500 以内
    console.log(`[chat] 回复超长（${text.length} 字符），触发摘要压缩`);
    try {
      const summary = await callLlm(
        [
          { role: "system", content: SUMMARY_PROMPT },
          { role: "user", content: text },
        ],
        2000,
      );
      if (summary && summary.length <= MAX_REPLY_CHARS) return summary;
      if (summary) return summary.slice(0, MAX_REPLY_CHARS);
      // 摘要为空 → 截断原文
      return text.slice(0, MAX_REPLY_CHARS);
    } catch (err) {
      console.error("[chat] 摘要失败，截断原文:", err);
      return text.slice(0, MAX_REPLY_CHARS);
    }
  } catch (err) {
    console.error(
      `[chat] 回退桩：LLM 调用异常（model=${LLM_MODEL} timeout=${LLM_TIMEOUT_MS}ms）`,
      err,
    );
    return fallback(content);
  }
}

const NARRATION_SYSTEM_PROMPT =
  "你是桌游群里的游戏主持人。把玩家的操作与游戏结果润色成一句简短、有情景带入感的话，承上启下推进游戏进程。不要逐条复述所有结果（详情玩家会去网页看），只做承上启下 + 情景带入。中文，不超过 200 字，直接输出正文。";

/** 游戏情景叙述：把动作 + 结果润色为承上启下、有带入感的短叙述。失败抛错（调用方静默降级为仅模板）。 */
export async function narrateGame(action: string, result: string): Promise<string> {
  if (!LLM_API_KEY) throw new Error("LLM 未配置，跳过情景叙述");
  const text = await callLlm(
    [
      { role: "system", content: NARRATION_SYSTEM_PROMPT },
      { role: "user", content: `玩家操作：${action}\n游戏结果：${result}` },
    ],
    2000,
  );
  if (!text) throw new Error("LLM 返回空");
  return text.length <= MAX_REPLY_CHARS ? text : text.slice(0, MAX_REPLY_CHARS);
}
