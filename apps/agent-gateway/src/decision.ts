//! 意图决策（本次为 mock，未接 SystemOne）。
//! 双通道交叉验证：决策模型为主，前缀作为低置信兜底。

export type Intent = "chat" | "start_game" | "game_action" | "dev";

export interface Decision {
  intent: Intent;
  confidence: number;
  /** 最终意图来源：decision=决策模型 / prefix=前缀兜底 / default=默认 */
  via: "decision" | "prefix" | "default";
}

const CONFIDENCE_THRESHOLD = 0.6;

// 前缀词表：#play / #start 对猜拳等价 → start_game（引擎需注意）
const PREFIX_RULES: Array<{ re: RegExp; intent: Intent }> = [
  { re: /#(聊|chat)/i, intent: "chat" },
  { re: /#(玩|play|start|开)/i, intent: "start_game" },
  { re: /#(开发|dev|develop)/i, intent: "dev" },
];

// mock 决策关键词（后续替换为 SystemOne 调用）
const RPS = /猜拳|石头剪刀布|rock[\s-]*paper[\s-]*scissors|rps/i;
const HAND = /石头|剪刀|布|✊|✌|✋/;
const DEV = /开发|部署|实现|修复|dev|deploy/i;

function parsePrefix(content: string): Intent | null {
  for (const rule of PREFIX_RULES) {
    if (rule.re.test(content)) return rule.intent;
  }
  return null;
}

/** mock 决策：明确关键词 → 高置信；其余 → chat 低置信。 */
function mockDecide(content: string): { intent: Intent; confidence: number } {
  if (DEV.test(content)) return { intent: "dev", confidence: 0.9 };
  if (RPS.test(content)) return { intent: "start_game", confidence: 0.9 };
  if (HAND.test(content)) return { intent: "game_action", confidence: 0.85 };
  return { intent: "chat", confidence: 0.5 };
}

/** 提取玩家手势（与引擎模拟决策呼应，仅提取传参）。 */
export function extractHand(content: string): string | null {
  const m = content.match(/石头|剪刀|布|✊|✌|✋|rock|paper|scissors/i);
  return m ? m[0] : null;
}

/** 双通道交叉验证入口。 */
export function decideIntent(content: string): Decision {
  const prefix = parsePrefix(content);
  const decision = mockDecide(content);

  if (decision.confidence >= CONFIDENCE_THRESHOLD) {
    return { intent: decision.intent, confidence: decision.confidence, via: "decision" };
  }
  if (prefix) {
    return { intent: prefix, confidence: decision.confidence, via: "prefix" };
  }
  return { intent: "chat", confidence: decision.confidence, via: "default" };
}
