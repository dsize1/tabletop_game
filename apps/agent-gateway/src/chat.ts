//! 聊天分支：接入 DeepSeek LLM（OpenAI 兼容接口）。
//! 超时/失败回退到回显桩，保证「必有反馈」。

const LLM_BASE_URL = process.env.LLM_BASE_URL ?? "https://api.deepseek.com";
const LLM_API_KEY = process.env.LLM_API_KEY ?? "";
const LLM_MODEL = process.env.LLM_MODEL ?? "deepseek-v4-flash";
const LLM_TIMEOUT_MS = Number(process.env.LLM_TIMEOUT_MS ?? 3000);
const LLM_MAX_TOKENS = Number(process.env.LLM_MAX_TOKENS ?? 1000);

const SYSTEM_PROMPT = "你是桌游群里的助手机器人，回复简洁、友好、口语化，用中文。";

/** 回显桩兜底：LLM 不可用时的降级回复。 */
function fallback(content: string): string {
  return `[聊天桩] 收到：${content}`;
}

/** 聊天回复：优先 LLM，超时/失败回退回显。 */
export async function chatReply(content: string): Promise<string> {
  if (!LLM_API_KEY) {
    return fallback(content);
  }

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
        messages: [
          { role: "system", content: SYSTEM_PROMPT },
          { role: "user", content },
        ],
        max_tokens: LLM_MAX_TOKENS,
        stream: false,
      }),
      signal: controller.signal,
    });

    if (!resp.ok) {
      const body = await resp.text().catch(() => "");
      console.error(`[chat] LLM HTTP ${resp.status}: ${body.slice(0, 200)}`);
      return fallback(content);
    }

    const data = (await resp.json()) as {
      choices?: Array<{ message?: { content?: string } }>;
    };
    const text = data.choices?.[0]?.message?.content?.trim();
    return text || fallback(content);
  } catch (err) {
    console.error("[chat] LLM 调用异常，回退回显:", err);
    return fallback(content);
  } finally {
    clearTimeout(timer);
  }
}
