#!/usr/bin/env node
// B8–B20 迁移脚手架：读对齐单 + v2 注册表，输出「这批要做什么/缺什么/怎么验」。
//
// 不写任何文件（安全），只打印清单；可选 --json 给机器读。
//
// 用法：
//   node apps/scripts/sgs2-batch-scaffold.mjs B8
//   node apps/scripts/sgs2-batch-scaffold.mjs B8 --json

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { projectRoot } from "./lib/sgs-catalog.mjs";

/**
 * 内核能力表（mechanism id → 是否已具备 + 落点）。
 * 迁移一批前先看这里：对齐单声明了 `⚠️` 的机制 = 本批必须先补内核。
 */
const MECHANISM_SUPPORT = {
  "card-attributes": { ok: true, where: "`cards::def()`（生成物）" },
  "card-target-attribute": { ok: true, where: "`card_play::planned_targets` + `skills::can_target`" },
  "target-list": { ok: true, where: "`card_play::target_list`（多目标循环）" },
  "target-extra": { ok: true, where: "`skills::extra_targets` + `advance_target`（B8）" },
  "card-gained": { ok: true, where: "`Game::gain`（`CardGained`）" },
  "card-out-in-game": { ok: true, where: "`Game::to_void/from_void` + `void_tags`" },
  "chained-state": { ok: true, where: "`flow::chain_enter/chain_leave`" },
  "damage-vs-hp-loss": { ok: true, where: "`flow::damage*` vs `flow::lose_hp`" },
  "deck-carrier": { ok: true, where: "`card_play::plan_use` 允许 `Zone::Draw` 承载（B8）" },
  "deferred-timing": { ok: true, where: "`kernel/deferred.rs`（单一延迟队列）" },
  "distance-modifier": { ok: true, where: "`Game::distance/attack_range/can_reach`" },
  "dying-resolution": { ok: true, where: "`flow::start_dying/step_dying` + `dying_hooks`" },
  "hand-freeze": { ok: true, where: "`Game::freeze_hand/freeze_cards_for_phase`" },
  "judge-zone": { ok: true, where: "`Game::to_judge` + `judge_zone_lifo`" },
  "lose-vs-discard": { ok: true, where: "`Game::to_discard` / `gain` / 判定区例外" },
  "pindian": { ok: true, where: "`kernel/pindian.rs` + `skills::pindian_*`（B8）" },
  "reveal": { ok: true, where: "`Game::reveal/reveal_private/conceal`" },
  "skill-card-zone": { ok: true, where: "`Zone::Mark` + `Game::to_mark/mark_cards`" },
  "skill-status": { ok: true, where: "`skills::active()`（自带 ∪ 授予）" },
  "transform-skill-windows": { ok: true, where: "`SkillDef.yang_window/yin_window`" },
  "trigger-order": { ok: true, where: "`event::sort_subs` + `Sub.priority`" },
  "turn-window": { ok: true, where: "`Window::OwnTurn/OwnPhase/OtherTurn/AnyTime`" },
  "use-vs-play": { ok: true, where: "`resolve_use` / `apply_response`（打出）" },
  "virtual-card": { ok: true, where: "`Game::use_virtual/use_as` + `virtual_defs`" },
  "wording-can": { ok: true, where: "技能可发动的 `can_active` / pending 出口" },
  "wording-then": { ok: true, where: "`Step::Next` 阶段推进（禁止同帧连做）" },
  "zone-ownership": { ok: true, where: "`zone` 唯一归属表 + 守恒" },
  // 尚未建模的机制（新增技能引用时会在这里报缺口）
  "damage-attributes": { ok: true, where: "`cards::DamageKind` + `flow::damage_typed`" },
  "response-cards": { ok: true, where: "`cards::ResponseKind` + `Game.response_context`" },
  "dying-aftermath": { ok: true, where: "`Game::after_dying_damage` + `skills::settle_after_dying`" },
  "discard-contract": { ok: true, where: "`Game::to_discard`（判定区不算弃置）" },
};

const args = process.argv.slice(2);
const batchArg = args.find((a) => /^B\d+$/i.test(a));
const jsonOut = args.includes("--json");
if (!batchArg) {
  console.error("用法：node apps/scripts/sgs2-batch-scaffold.mjs B8 [--json]");
  process.exit(2);
}
const batch = batchArg.toUpperCase();
const batchNum = Number(batch.slice(1));

const file = path.join(projectRoot, "docs/games/sanguosha/align", `${batch}.md`);
if (!existsSync(file)) {
  console.log(`[sgs2:batch] 缺少对齐单：${path.relative(projectRoot, file)}`);
  console.log("先创建骨架（6 段内容由 agent 填，第 6/7 段由用户确认）：");
  console.log(`  node apps/scripts/sgs-align-sheet.mjs --batch ${batch} --new`);
  console.log("然后重新运行：");
  console.log(`  npm run sgs2:batch -- ${batch}`);
  process.exit(0);
}
const text = readFileSync(file, "utf8");

// 注册表快照
const run = spawnSync(
  "cargo",
  [
    "run",
    "-q",
    "--manifest-path",
    "crates/Cargo.toml",
    "-p",
    "sanguosha2",
    "--example",
    "sgs2-registry",
    "--",
    "--json",
  ],
  { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 },
);
if (run.status !== 0) {
  console.error("[sgs2:batch] 注册表导出失败：");
  console.error(run.stderr?.slice(0, 1500) ?? "");
  process.exit(1);
}
const line = (run.stdout ?? "")
  .split(/\r?\n/)
  .map((s) => s.trim())
  .find((s) => s.startsWith("{"));
const reg = JSON.parse(line);
const registered = new Set(reg.skills.map((s) => s.code));

// 对齐单：武将 + 技能
const generals = [...text.matchAll(/^#\s+(.+?)\s+`(sgs_hero_\d+)`（(\d+)\s*技能）/gm)].map((m) => ({
  name: m[1],
  code: m[2],
  skillCount: Number(m[3]),
}));
const skills = [...text.matchAll(/^##\s+([^（(]+)（`([a-z0-9_]+)`\s*\/\s*([a-z]+)）(.*)$/gm)].map((m) => ({
  name: m[1],
  code: m[2],
  kind: m[3],
  implementedInDoc: m[4].includes("implemented"),
  inRegistry: registered.has(m[2]),
}));

const missing = skills.filter((s) => !s.inRegistry);
const testFile = path.join(projectRoot, "crates/engines/sanguosha2/tests", `${batch.toLowerCase()}.rs`);
const skillFile = path.join(projectRoot, "crates/engines/sanguosha2/src/skills", `${batch.toLowerCase()}.rs`);

// 对齐单 JSON：疑点未答数 / status / 机制声明（门禁前先看这里）
const specPath = path.join(projectRoot, "docs/games/sanguosha/align", `${batch}.json`);
const spec = existsSync(specPath) ? JSON.parse(readFileSync(specPath, "utf8")) : null;
const alignQuestions = spec
  ? spec.generals.flatMap((g) => g.skills).flatMap((k) => k.questions || [])
  : [];
const unanswered = alignQuestions.filter((q) => !q.answer);
const declaredMechanics = [
  ...new Set(
    (spec ? spec.generals.flatMap((g) => g.skills).flatMap((k) => k.mechanics || []) : []),
  ),
];
const mechGaps = declaredMechanics
  .map((id) => ({ id, support: MECHANISM_SUPPORT[id] }))
  .filter((m) => !m.support || !m.support.ok);
const mechUnknown = declaredMechanics.filter((id) => !MECHANISM_SUPPORT[id]);
const blocked = spec && spec.status !== "confirmed" && spec.status !== "implemented";

const plan = {
  batch,
  batchNum,
  generals,
  skills,
  align: spec
    ? {
        status: spec.status,
        questions: alignQuestions.length,
        unanswered: unanswered.length,
        unansweredList: unanswered.map((q) => q.id),
        mechanics: declaredMechanics,
        mechanismGaps: mechGaps,
        mechanismUnknown: mechUnknown,
        ready: !blocked && unanswered.length === 0 && mechGaps.length === 0 && mechUnknown.length === 0,
      }
    : null,
  missing: missing.map((s) => s.code),
  files: {
    align: path.relative(projectRoot, file),
    skills: path.relative(projectRoot, skillFile),
    tests: path.relative(projectRoot, testFile),
    skillsExists: existsSync(skillFile),
    testsExists: existsSync(testFile),
  },
  next: [
    `1. 在 ${path.relative(projectRoot, skillFile)} 实现技能，并在 src/skills/mod.rs 的 SKILLS 注册`,
    `2. 在 ${path.relative(projectRoot, testFile)} 加正/反例（每技能 ≥1 正例 + ≥1 反例）`,
    `3. 把 ${batchNum} 加入 apps/scripts/check-sgs2-registry.mjs 的 V2_BATCHES（登记「这批已迁到 v2」）`,
    "4. 回写对齐单第 2 段（现实现）为新引擎行为 + impl 锚点",
    "5. 运行 npm run check:sgs2 && npm run sgs:check:fast",
  ],
};

if (jsonOut) {
  console.log(JSON.stringify(plan, null, 2));
  process.exit(missing.length && !plan.files.testsExists ? 0 : 0);
}

console.log(`\n== ${batch} · ${generals.length} 将 / ${skills.length} 技能 ==`);
for (const g of generals) {
  console.log(`  ${g.code}  ${g.name}（${g.skillCount} 技能）`);
}
console.log("\n技能 → v2 注册状态：");
for (const s of skills) {
  const mark = s.inRegistry ? "✅ 已注册" : "⬜ 未注册";
  console.log(`  ${mark}  ${s.code.padEnd(14)} ${s.name}（${s.kind}）`);
}
if (missing.length) {
  console.log(`\n待迁移（${missing.length}）：${missing.map((s) => s.code).join(", ")}`);
} else {
  console.log("\n全部技能已注册（本批可做「回写对齐单 + 用例复查」）。");
}
if (plan.align) {
  const a = plan.align;
  console.log(
    `\n对齐单：status=${a.status}｜疑点 ${a.questions} 条（未答 ${a.unanswered}${a.unanswered ? `：${a.unansweredList.join(",")}` : ""}）`,
  );
  console.log(`机制依赖：${a.mechanics.length} 条`);
  for (const m of a.mechanismGaps) {
    console.log(`  ⚠️ 内核缺口  ${m.id}（${m.support ? m.support.where : "未登记"}）`);
  }
  for (const m of a.mechanismUnknown) {
    console.log(`  ❓ 未登记机制  ${m}（请先在 MECHANISM_SUPPORT 里登记能力/落点）`);
  }
  if (a.status !== "confirmed" && a.status !== "implemented") {
    console.log(`  ⛔ 对齐单未确认（status=${a.status}）：先走「疑点裁定 → 第 7 段」再实现`);
  }
  if (a.unanswered) console.log("  ⛔ 仍有未裁定疑点：实现前必须全部有 answer");
  if (a.ready) console.log("  ✅ 可开工（已对齐 + 内核能力齐备）");
}
console.log("\n文件：");
console.log(`  对齐单：${plan.files.align}`);
console.log(`  技能：  ${plan.files.skills}${plan.files.skillsExists ? "" : "（待创建）"}`);
console.log(`  用例：  ${plan.files.tests}${plan.files.testsExists ? "" : "（待创建）"}`);
console.log("\n下一步：");
for (const n of plan.next) console.log(`  ${n}`);
