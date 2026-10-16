//! SystemOne 把关器（ADR-0007 决策 13）：判定候选事实是否写入长期画像。
//! 生成归本地 9B、把关归决策模型；把关失败默认保留（不因故障丢信息）。

import type { SystemOneClient } from "../systemone.js";

export type ProfileGate = (candidate: string, oldProfile: string) => Promise<boolean>;

const GATE_INSTRUCTIONS =
  "这条候选事实是否值得写入该用户的长期人物画像？" +
  "保留：稳定的**人物信息**——称呼/昵称/外号、口头习惯、喜好口味、职业/身份线索、稳定性格；" +
  "拒绝：临时情绪、当下场景、一次性事件、话题内容、技术/业务细节。";

/** 构造把关函数（阈值：P(值得写入) ≥ threshold 才保留）。 */
export function makeSystemOneGate(
  systemone: SystemOneClient,
  threshold: number,
): ProfileGate {
  return async (candidate: string, oldProfile: string): Promise<boolean> => {
    try {
      const resp = await systemone.decide(
        { candidate, old_profile: oldProfile },
        { worth: { type: "noul", instructions: GATE_INSTRUCTIONS } },
      );
      const ans = resp.answers.worth;
      if (ans && ans.type === "noul") return ans.noul >= threshold;
    } catch (err) {
      console.error("[profile] 把关失败，默认保留:", err);
    }
    return true;
  };
}
