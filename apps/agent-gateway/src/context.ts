//! 上下文来源按线抽象（ADR-0007 决策 20）。
//!
//! 把 chat / game 各自的「背景组装」集中在这里：未来「砍 chat + 放开非 @ → 两线合并」
//! 时只改本文件，不动 agent 编排（`index.ts`）与 LLM 客户端（`chat.ts`）。
//!
//! **两条线的背景内容互相隔离**（ADR-0007 决策 8）：chat 用阶段历史（Redis 热缓存按人隔离、
//! 组装前按 `boundary` 裁到当前阶段，见 `chat-history.ts`），game 用局面摘要（引擎 `state_summary`）。
//! **画像（全局层）两条线共享**（决策 9）。

import { formatTimeBlock, lastTurnTs, renderHistory, type TimedTurn } from "./time-context.js";
import { buildProfileBlock } from "./profile/inject.js";

/**
 * 全局人格（所有线共享）。
 * 注意：改动此处会击穿 DeepSeek 前缀缓存（每次 miss），非必要不改。
 */
export const SYSTEM_PROMPT =
  "你是桌游群里的助手机器人，回复简洁、友好、口语化，用中文。" +
  "除非用户主动问，不要在回复里提时间（不要说现在几点、今天几号）；" +
  "上下文里出现的时间信息只用于你自己判断对话隔了多久。";

export interface ChatContextOptions {
  /** 当前时刻（epoch ms），便于测试注入假时钟。 */
  now?: number;
  /** 时区覆盖（默认 `time-context.CHAT_TZ`）。 */
  tz?: string;
  /** 全局人物画像（跨群；ADR-0007）；无则跳过注入。 */
  profile?: string;
}

/**
 * chat 线背景组装（纯函数，便于单测）：
 * `system` 人设 → **全局画像** → 阶段历史（簇首带时间锚点、边界带话题切换标记）→ `system` 时间块 → 当前用户消息。
 *
 * 注意「时间只入上下文」：时间块是独立条目，用户消息与历史 `content` 均原样不改写。
 */
export function buildChatContext(
  content: string,
  history: readonly TimedTurn[] = [],
  opts: ChatContextOptions = {},
): Array<{ role: string; content: string }> {
  const now = opts.now ?? Date.now();
  return [
    { role: "system", content: SYSTEM_PROMPT },
    ...buildProfileBlock(opts.profile),
    ...renderHistory(history, { tz: opts.tz }),
    { role: "system", content: formatTimeBlock(now, lastTurnTs(history), opts.tz) },
    { role: "user", content },
  ];
}

/** game 线情景叙述的 system 提示。 */
export const GAME_NARRATION_SYSTEM_PROMPT =
  "你是桌游群里的游戏主持人。把玩家的操作与游戏结果润色成一句简短、有情景带入感的话，承上启下推进游戏进程。不要逐条复述所有结果（详情玩家会去网页看），只做承上启下 + 情景带入。中文，不超过 200 字，直接输出正文。" +
  // 主体钉死（bug 修复）：局面摘要描述的是**动作之后**的局面，「等待 X」是下一位，曾被模型当成执行者。
  "叙述主体必须是「执行玩家」（本次动作的执行者）；「当前局面」里出现的「等待 X」是下一步轮到的玩家，不要写成他执行了本次动作。";

export interface GameNarrationInput {
  /** 本次动作的执行者展示名（叙述主体；缺省则不给主体提示） */
  actor?: string;
  /** 本次玩家操作 */
  action: string;
  /** 规则引擎结果文案（模板摘要） */
  result: string;
  /** 引擎局面摘要（game 线背景；ADR-0007 决策 7） */
  stateSummary?: string;
  /** 玩家全局画像（全局层；ADR-0007 决策 9） */
  profileText?: string;
}

/**
 * game 线背景组装：局面摘要 + 玩家画像 + 操作 + 结果（作为叙述 LLM 的 user 内容）。
 * 只作上下文，不逐字复述。
 */
export function buildGameNarrationInput(a: GameNarrationInput): string {
  const lines: string[] = [];
  if (a.actor) lines.push(`执行玩家：${a.actor}`);
  // 摘要是「动作已生效后」的局面：显式标注，避免把「等待 X」误当执行者
  if (a.stateSummary)
    lines.push(
      `当前局面（本次动作已生效；其中「等待 X」是下一步轮到的玩家，不是本次动作执行者）：${a.stateSummary}`,
    );
  if (a.profileText) lines.push(`玩家画像：${a.profileText}`);
  lines.push(`玩家操作：${a.action}`);
  lines.push(`游戏结果：${a.result}`);
  return lines.join("\n");
}
