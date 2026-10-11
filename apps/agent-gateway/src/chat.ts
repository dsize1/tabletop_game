//! 聊天分支：接入 DeepSeek LLM（OpenAI 兼容接口）。
//! 回复限长 1500 字符：超长时模型多做一步「摘要压缩」，保证回复不超限。

const LLM_BASE_URL = process.env.LLM_BASE_URL ?? "https://api.deepseek.com";
const LLM_API_KEY = process.env.LLM_API_KEY ?? "";
const LLM_MODEL = process.env.LLM_MODEL ?? "deepseek-flash";
const LLM_TIMEOUT_MS = Number(process.env.LLM_TIMEOUT_MS ?? 30000);
const LLM_MAX_TOKENS = Number(process.env.LLM_MAX_TOKENS ?? 2000);

/** 回复最大字符数（QQ 消息有长度限制，超限会报 40054007）。 */
const MAX_REPLY_CHARS = 1500;

const SYSTEM_PROMPT = "你是桌游群里的助手机器人，回复简洁、友好、口语化，用中文。";

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
        // 关闭思考模式：直接输出 content，不产生 reasoning_content（快、省 token）
        thinking: { type: "disabled" },
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

/** 聊天回复：正常 LLM 回复；超 1500 字符时再做一步摘要压缩。 */
export async function chatReply(content: string): Promise<string> {
  if (!LLM_API_KEY) {
    return fallback(content);
  }

  try {
    // 第一段：正常回复
    const text = await callLlm(
      [
        { role: "system", content: SYSTEM_PROMPT },
        { role: "user", content },
      ],
      LLM_MAX_TOKENS,
    );
    if (!text) return fallback(content);
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
    console.error("[chat] LLM 调用异常，回退回显:", err);
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
