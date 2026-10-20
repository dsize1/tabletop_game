// 上下文来源按线抽象单测：chat 线（人格→画像→历史→时间块→当前）、game 线（局面+画像+操作+结果）。
// 运行：npx tsx test-context.mts
import assert from "node:assert";
import {
  GAME_NARRATION_SYSTEM_PROMPT,
  SYSTEM_PROMPT,
  buildChatContext,
  buildGameNarrationInput,
} from "./src/context.js";

// game 线 system 提示：主体钉死为执行玩家（防「等待 X」被误当执行者）
assert.ok(GAME_NARRATION_SYSTEM_PROMPT.includes("执行玩家"));
assert.ok(GAME_NARRATION_SYSTEM_PROMPT.includes("不要写成他执行了本次动作"));

// chat 线：人格 → 画像 → 历史 → 时间块 → 当前消息
{
  const history = [{ role: "user" as const, content: "旧消息", ts: 1 }];
  const msgs = buildChatContext("现在说的话", history, {
    now: 2,
    tz: "Asia/Shanghai",
    profile: "叫华哥；喜欢竞技",
  });
  assert.equal(msgs[0]!.content, SYSTEM_PROMPT, "首条为全局人格");
  assert.equal(msgs[1]!.role, "system");
  assert.ok(msgs[1]!.content.includes("叫华哥"), "注入画像块");
  assert.ok(msgs.some((m) => m.content.includes("旧消息")), "含历史");
  const last = msgs[msgs.length - 1]!;
  assert.equal(last.role, "user");
  assert.equal(last.content, "现在说的话");
}

// 无画像 → 不注入画像块
{
  const msgs = buildChatContext("hi", [], { now: 2 });
  assert.ok(!msgs.some((m) => m.content.includes("[人物画像]")));
  assert.equal(msgs[msgs.length - 1]!.content, "hi");
}

// game 线：执行者 + 局面 + 画像 + 操作 + 结果
{
  const s = buildGameNarrationInput({
    actor: "轮子总比困难多",
    action: "出石头",
    result: "你赢了",
    stateSummary: "轮到 A",
    profileText: "叫华哥",
  });
  assert.ok(s.startsWith("执行玩家：轮子总比困难多"), "首行钉死叙述主体");
  assert.ok(s.includes("当前局面（本次动作已生效"), "摘要标注为动作后局面");
  assert.ok(s.includes("轮到 A"));
  assert.ok(s.includes("玩家画像：叫华哥"));
  assert.ok(s.includes("玩家操作：出石头"));
  assert.ok(s.includes("游戏结果：你赢了"));
  // 无可选背景时只留操作+结果
  assert.equal(buildGameNarrationInput({ action: "a", result: "r" }), "玩家操作：a\n游戏结果：r");
}

console.log("✅ context 单测全部通过");
