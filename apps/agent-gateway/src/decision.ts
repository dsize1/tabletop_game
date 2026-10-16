//! 意图决策：决策模型 SystemOne 为主，`#前缀` / 默认 chat 兜底。
//! SystemOne 不可用（超时/连不上/报错）单独标记 `via="unavailable"`，由调用方回「服务繁忙」。
import type { Question, SystemOneClient } from "./systemone.js";

export type Intent = "chat" | "start_game" | "game_action" | "dev";

export interface Decision {
  intent: Intent;
  confidence: number;
  /**
   * 最终意图来源：
   * - decision=决策模型（≥ 高阈值，直接采用）
   * - confirm=中带置信度，待人工确认（候选见 `model`；`intent` 与 `model.intent` 一致，调用方须先用 `via` 分流）
   * - prefix=前缀兜底 / default=默认 / unavailable=决策服务不可用
   */
  via: "decision" | "confirm" | "prefix" | "default" | "unavailable";
  /**
   * 决策模型的原始输出（有回答时）：候选意图 + 置信度。
   * 不论最终走哪条支路都保留，用于日志「结果 + 置信度」回溯与是否进过确认阶段的推断。
   */
  model?: { intent: Intent; confidence: number };
  /**
   * 话题切换的 P(true)（仅在 `askTopicSwitch` 时请求；0~1）。
   * 门控与阈值判定在调用方（`index.ts`）；`undefined` 表示未询问 / 无有效回答 / 决策服务不可用。
   */
  topicSwitch?: number;
}

/** 意图置信度默认低阈值（低于 → `#前缀` → 默认 chat；可被 `DecideOptions.threshold` 覆盖）。 */
export const DEFAULT_CONFIDENCE_THRESHOLD = 0.6;
/** 意图置信度默认高阈值（≥ 直接采用；`[低, 高)` 走人工确认；可被 `DecideOptions.thresholdHigh` 覆盖）。 */
export const DEFAULT_CONFIDENCE_THRESHOLD_HIGH = 0.8;

export type ConfidenceBand = "high" | "confirm" | "low";

/** 置信度分档：约定 `[low, high)` 为 confirm（左闭右开）。 */
export function confidenceBand(confidence: number, low: number, high: number): ConfidenceBand {
  if (confidence >= high) return "high";
  if (confidence >= low) return "confirm";
  return "low";
}

/** 规整阈值：非数值回退默认；高阈值倒挂时夹紧为低阈值并告警。 */
export function normalizeThresholds(low: number, high: number): { low: number; high: number } {
  const lo = Number.isFinite(low) ? low : DEFAULT_CONFIDENCE_THRESHOLD;
  let hi = Number.isFinite(high) ? high : DEFAULT_CONFIDENCE_THRESHOLD_HIGH;
  if (hi < lo) {
    console.warn(`[decision] 高阈值(${hi}) < 低阈值(${lo})，已夹紧为 ${lo}`);
    hi = lo;
  }
  return { low: lo, high: hi };
}

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

/**
 * 意图分类前屏蔽 URL：链接文本（如 `/dev_docs`）会被决策模型当作强 dev 信号，导致误判。
 * 仅用于送模型的 state；`#前缀` 兜底与链接抓取判断仍用原文。
 */
export function maskUrls(content: string): string {
  return content.replace(/https?:\/\/[^\s<>"'）)】\]，。；：！？、（）【】《》]+/gi, "[链接]");
}

export interface DecideOptions {
  /** 低置信度阈值，低于则走前缀/默认兜底。 */
  threshold?: number;
  /** 高置信度阈值，`[threshold, thresholdHigh)` 视为待人工确认。 */
  thresholdHigh?: number;
  /** 覆盖意图选项描述（默认 `INTENT_CRITERIA`）。 */
  intentCriteria?: Record<Intent, string>;
  /**
   * 是否追加 `topic_switch` 问题（由调用方按「距上次对话间隔」门控）。
   * 追加后仍是**同一次** HTTP 请求 / 同一次前向，不增加往返延迟。
   */
  askTopicSwitch?: boolean;
  /** 上一条用户消息内容（`topic_switch` 判定用；无则不传）。 */
  previousMessage?: string;
}

/** 提取消息中的 https:// 链接（必须有协议头；剔除尾部中英文标点，去重）。 */
export function extractHttpsLinks(content: string): string[] {
  const re = /https:\/\/[^\s<>"'）)】\]，。；：！？、（）【】《》]+/gi;
  const out: string[] = [];
  for (const m of content.matchAll(re)) {
    const url = m[0].replace(/[.,;:!?、。，；：！？]+$/u, "");
    if (url.length > "https://".length && !out.includes(url)) out.push(url);
  }
  return out;
}

/** 链接抓取判断的默认阈值（P(true) ≥ 阈值则抓取）。 */
export const DEFAULT_LINK_FETCH_THRESHOLD = 0.5;

/** 话题切换阈值：P(true) ≥ 阈值才判「新话题」（保守取值，宁少标、不误切）。 */
export const DEFAULT_TOPIC_SWITCH_THRESHOLD = 0.7;

/** 话题切换问题（`noul` 是/否；并入 `decideIntent` 的同一次前向，零额外 RTT）。 */
export const TOPIC_SWITCH_INSTRUCTIONS = "当前这条消息是在开启一个新话题，还是在延续上一次的对话？";
export const TOPIC_SWITCH_CRITERIA: { true: string; false: string } = {
  true: "新话题：与上一次内容无关，或明显是隔了一段时间后的新对话（打招呼、换事情）",
  false: "延续：仍在接着上一次的内容说",
};

/** 话题切换判定：P(true) ≥ 阈值算新话题；未询问 / 无有效回答 → false。 */
export function isTopicSwitch(
  topicSwitch: number | undefined,
  threshold: number = DEFAULT_TOPIC_SWITCH_THRESHOLD,
): boolean {
  return (
    typeof topicSwitch === "number" &&
    Number.isFinite(topicSwitch) &&
    topicSwitch >= threshold
  );
}

export interface LinkFetchDecision {
  /** 是否需要抓取链接内容 */
  fetch: boolean;
  /** 模型给出的 P(true)（`noul` 是/否题）。 */
  probability: number;
}

/**
 * 用 SystemOne `noul`（是/否）判断：这条消息里的链接是否需要抓取正文才能回答。
 * `noul` 只返回 P(true)（无 confidence/probabilities），因此用阈值判否；
 * 服务不可用/超时/返回异常一律判「不抓取」（安全降级）。
 */
export async function decideLinkFetch(
  content: string,
  links: string[],
  client: SystemOneClient,
  opts: { threshold?: number } = {},
): Promise<LinkFetchDecision> {
  const threshold = opts.threshold ?? DEFAULT_LINK_FETCH_THRESHOLD;
  try {
    const res = await client.decide(
      { message: content, links },
      {
        need_fetch: {
          type: "noul",
          instructions: "这条消息里的链接，需要抓取网页内容才能回答用户吗？",
          criteria: {
            true: "需要：用户在询问或要求解读链接里的内容",
            false: "不需要：链接只是附带分享，或与用户的问题无关",
          },
        },
      },
    );
    const a = res.answers?.need_fetch;
    if (a && a.type === "noul" && typeof a.noul === "number") {
      return { fetch: a.noul >= threshold, probability: a.noul };
    }
  } catch (err) {
    console.error("[decision] 链接抓取判断失败:", (err as Error)?.message ?? err);
  }
  return { fetch: false, probability: 0 };
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
  const { low, high } = normalizeThresholds(
    opts.threshold ?? DEFAULT_CONFIDENCE_THRESHOLD,
    opts.thresholdHigh ?? DEFAULT_CONFIDENCE_THRESHOLD_HIGH,
  );

  let answer: { choice: Intent; confidence: number } | null = null;
  let topicSwitch: number | undefined;
  try {
    // 意图与话题切换在**同一次**请求里多问（单次前向），不额外增加 RTT。
    const questions: Record<string, Question> = {
      intent: {
        type: "choice",
        instructions: "玩家的意图是什么？",
        criteria: opts.intentCriteria ?? INTENT_CRITERIA,
      },
    };
    const state: Record<string, unknown> = { message: maskUrls(content) };
    if (opts.askTopicSwitch) {
      if (opts.previousMessage) state.previous_message = maskUrls(opts.previousMessage);
      questions.topic_switch = {
        type: "noul",
        instructions: TOPIC_SWITCH_INSTRUCTIONS,
        criteria: TOPIC_SWITCH_CRITERIA,
      };
    }

    const res = await client.decide(state, questions);
    const a = res.answers?.intent;
    if (a && a.type === "choice" && isIntent(a.choice)) {
      answer = { choice: a.choice, confidence: a.confidence };
    }
    const t = res.answers?.topic_switch;
    if (t && t.type === "noul" && typeof t.noul === "number") topicSwitch = t.noul;
  } catch (err) {
    console.error("[decision] SystemOne 调用失败:", (err as Error)?.message ?? err);
    return { intent: "chat", confidence: 0, via: "unavailable" };
  }

  if (answer) {
    const model = { intent: answer.choice, confidence: answer.confidence };
    const band = confidenceBand(answer.confidence, low, high);
    if (band === "high") {
      return { intent: answer.choice, confidence: answer.confidence, via: "decision", model, topicSwitch };
    }
    // 中带且候选非 chat：交调用方人工确认（候选=chat 时确认无意义，等同低阈值兜底）。
    if (band === "confirm" && answer.choice !== "chat") {
      return { intent: answer.choice, confidence: answer.confidence, via: "confirm", model, topicSwitch };
    }
  }
  const model = answer ? { intent: answer.choice, confidence: answer.confidence } : undefined;
  const prefix = parsePrefix(content);
  if (prefix) {
    return { intent: prefix, confidence: answer?.confidence ?? 0, via: "prefix", model, topicSwitch };
  }
  return { intent: "chat", confidence: answer?.confidence ?? 0, via: "default", model, topicSwitch };
}
