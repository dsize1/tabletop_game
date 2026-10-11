//! 意图决策：决策模型 SystemOne 为主，`#前缀` / 默认 chat 兜底。
//! SystemOne 不可用（超时/连不上/报错）单独标记 `via="unavailable"`，由调用方回「服务繁忙」。
import type { SystemOneClient } from "./systemone.js";

export type Intent = "chat" | "start_game" | "game_action" | "dev";

export interface Decision {
  intent: Intent;
  confidence: number;
  /** 最终意图来源：decision=决策模型 / prefix=前缀兜底 / default=默认 / unavailable=决策服务不可用 */
  via: "decision" | "prefix" | "default" | "unavailable";
}

/** 意图置信度默认阈值（可被 `DecideOptions.threshold` 覆盖）。 */
export const DEFAULT_CONFIDENCE_THRESHOLD = 0.6;

// 前缀词表：#play / #start 对猜拳等价 → start_game（引擎需注意）。
// 注意顺序：「#开发」同时命中「开」，故 dev 规则须排在 start_game 之前。
const PREFIX_RULES: Array<{ re: RegExp; intent: Intent }> = [
  { re: /#(聊|chat)/i, intent: "chat" },
  { re: /#(开发|dev|develop)/i, intent: "dev" },
  { re: /#(玩|play|start|开)/i, intent: "start_game" },
];

/** 意图选项描述（中文；实测零样本跨语言可用）。 */
export const INTENT_CRITERIA: Record<Intent, string> = {
  chat: "闲聊、打招呼、普通提问，或与游戏操作无关的内容",
  start_game: "请求开始新游戏或新的一局",
  game_action: "在当前对局中出手势或声明一个动作",
  dev: "软件开发、部署或修复 bug 等开发任务",
};

const INTENTS: readonly Intent[] = ["chat", "start_game", "game_action", "dev"];
function isIntent(v: string): v is Intent {
  return (INTENTS as readonly string[]).includes(v);
}

/** 解析 `#前缀`，命中返回意图，否则 null。 */
export function parsePrefix(content: string): Intent | null {
  for (const rule of PREFIX_RULES) {
    if (rule.re.test(content)) return rule.intent;
  }
  return null;
}

/** 提取玩家手势（与引擎模拟决策呼应，仅提取传参）。 */
export function extractHand(content: string): string | null {
  const m = content.match(/石头|剪刀|布|✊|✌|✋|rock|paper|scissors/i);
  return m ? m[0] : null;
}

export interface DecideOptions {
  /** 置信度阈值，低于则走前缀/默认兜底。 */
  threshold?: number;
  /** 覆盖意图选项描述（默认 `INTENT_CRITERIA`）。 */
  intentCriteria?: Record<Intent, string>;
}

/**
 * 双通道交叉验证入口：SystemOne 决策为主；低置信 → `#前缀` → 默认 chat。
 * SystemOne 不可用时返回 `via="unavailable"`（不降级为 chat，交由调用方回「服务繁忙」）。
 */
export async function decideIntent(
  content: string,
  client: SystemOneClient,
  opts: DecideOptions = {},
): Promise<Decision> {
  const threshold = opts.threshold ?? DEFAULT_CONFIDENCE_THRESHOLD;

  let answer: { choice: Intent; confidence: number } | null = null;
  try {
    const res = await client.decide(
      { message: content },
      {
        intent: {
          type: "choice",
          instructions: "玩家的意图是什么？",
          criteria: opts.intentCriteria ?? INTENT_CRITERIA,
        },
      },
    );
    const a = res.answers?.intent;
    if (a && a.type === "choice" && isIntent(a.choice)) {
      answer = { choice: a.choice, confidence: a.confidence };
    }
  } catch (err) {
    console.error("[decision] SystemOne 调用失败:", (err as Error)?.message ?? err);
    return { intent: "chat", confidence: 0, via: "unavailable" };
  }

  if (answer && answer.confidence >= threshold) {
    return { intent: answer.choice, confidence: answer.confidence, via: "decision" };
  }
  const prefix = parsePrefix(content);
  if (prefix) {
    return { intent: prefix, confidence: answer?.confidence ?? 0, via: "prefix" };
  }
  return { intent: "chat", confidence: answer?.confidence ?? 0, via: "default" };
}
