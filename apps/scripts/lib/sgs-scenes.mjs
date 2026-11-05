// 技能场景骨架生成器（L2 台账用）。
//
// 边界（沿用全武将验证口径）：
// - 只生成**场景**（武将/手牌/装备/阶段/目标/seed）与操作提示；
// - **绝不生成 `expected`**（语义期望必须人工确认，agent 不得推断）；
// - 仅补齐台账中缺失的技能条目，已有条目（含人工裁定/确认人）保持不变。
//
// 生成策略按技能 kind：
//   normal     → 出牌阶段 + 固定手牌（杀/闪/桃/酒/决斗/过河/顺手/无懈/装备）+ 2 敌 2 友
//   locked     → 同上（仅用于机械证据；效果需人工断言）
//   lord       → 目标为主公身份 + 至少 1 名同势力友军（响应方）
//   limited    → 同上，附「每局一次」提示
//   awaken     → 同上，附「觉醒条件需人工构造」提示
//   其它/未知  → 同上，附「触发时机需人工确认」提示

export const L1_SEED = 20261105;
export const L1_HAND = [
  "sha",
  "sha",
  "shan",
  "tao",
  "jiu",
  "juedou",
  "guohe_chaiqiao",
  "shunshou_qianyang",
  "wuxie_keji",
  "zhuge_liannu",
];
export const L1_EQUIPS = { weapon: "zhuge_liannu", armor: "bagua_zhen" };
export const L1_OTHER_HAND = ["sha", "shan"];

const PHASE_HINT = {
  normal: "play",
  locked: "play",
  lord: "play",
  limited: "play",
  awaken: "prepare",
  trigger: "any",
  transform: "play",
};

/** 技能类型 → 场景骨架（可被审计脚本 `--with-scenes` 直接写入台账）。 */
export function sceneSkeleton(general, skill, players = 5) {
  const kind = skill.kind ?? "normal";
  const seats = [
    {
      index: 0,
      identity: kind === "lord" ? "lord" : "lord",
      hand: L1_HAND,
      equips: L1_EQUIPS,
      judge: [],
      chained: false,
      flipped: false,
    },
    ...Array.from({ length: players - 1 }, (_, i) => ({
      index: i + 1,
      hand: L1_OTHER_HAND,
    })),
  ];
  const notes = [];
  let actionHint = `skill:${skill.code}`;
  if (kind === "locked") {
    notes.push("锁定技：自动生效，无独立动作；机械证据只能证明不报错/守恒，效果需人工断言");
    actionHint = null;
  } else if (kind === "lord") {
    notes.push("主公技：需同势力角色响应；L1 未构造响应方 → 人工走查时补一名同势力友军");
  } else if (kind === "limited") {
    notes.push("限定技：每局一次；如已发动过需重置对局");
  } else if (kind === "awaken") {
    notes.push("觉醒技：需人工构造觉醒条件（标记/体力/阶段）");
    actionHint = null;
  } else if (kind === "trigger") {
    notes.push("触发技：需人工确认触发时机（准备/判定/摸牌/弃牌/结束/回合外）");
    actionHint = null;
  }
  notes.push(
    "`expected` 必须人工填写（语义期望不由生成器推断）；场景仅提供最小复现局面",
  );
  return {
    players,
    seed: L1_SEED,
    phaseHint: PHASE_HINT[kind] ?? "play",
    scene: {
      round: 1,
      turn: 0,
      turn_phase: PHASE_HINT[kind] ?? "play",
      phase: "turn",
      seed: L1_SEED,
      deck_top: ["sha", "shan", "tao", "wuxie_keji", "jiu", "juedou"],
      seats,
    },
    actionHint,
    notes,
  };
}

/** 生成台账条目（`expected` 恒为 null，等待人工确认）。 */
export function ledgerEntry(batchId, general, skill, players = 5) {
  const sk = sceneSkeleton(general, skill, players);
  return {
    batch: batchId,
    general: general.code,
    generalName: general.name,
    skill: skill.code,
    skillName: skill.name,
    kind: skill.kind ?? "normal",
    phaseHint: sk.phaseHint,
    action: sk.actionHint,
    scene: sk.scene,
    expected: null,
    expectedConfirmedBy: null,
    confirmedAt: null,
    generatedBy: "sgs-scenes/v1",
    notes: sk.notes.join("；"),
  };
}

/**
 * 合并台账：只为缺失的技能补骨架；已有条目（含人工 `expected`/确认人）原样保留。
 * 返回 { ledger, added: [{general, skill}], kept: number }。
 */
export function mergeScenarios(ledger, batchId, generals, players = 5) {
  const next = { ...ledger, skillTests: Array.isArray(ledger.skillTests) ? [...ledger.skillTests] : [] };
  const key = (e) => `${e.general}::${e.skill}`;
  const have = new Set(next.skillTests.map(key));
  const added = [];
  for (const g of generals) {
    for (const s of g.skills ?? []) {
      const k = `${g.code}::${s.code}`;
      if (have.has(k)) continue;
      next.skillTests.push(ledgerEntry(batchId, g, s, players));
      have.add(k);
      added.push({ general: g.name, skill: s.name, code: s.code });
    }
  }
  return { ledger: next, added, kept: next.skillTests.length - added.length };
}
