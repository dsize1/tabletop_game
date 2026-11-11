#!/usr/bin/env node
// 内核 v2 度量（spike 报告用，非门禁）：新 crate 的 LOC/用例 vs 旧引擎「同技能专属代码」。
//
// 用法：node apps/scripts/check-sgs2-metrics.mjs [--verbose]

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { projectRoot } from "./lib/sgs-catalog.mjs";

const NEW_SRC = path.join(projectRoot, "crates/engines/sanguosha2/src");
const NEW_TESTS = path.join(projectRoot, "crates/engines/sanguosha2/tests");
const OLD_SRC = path.join(projectRoot, "crates/engines/sanguosha/src");

function walk(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = path.join(dir, f);
    return statSync(p).isDirectory() ? walk(p) : p.endsWith(".rs") ? [p] : [];
  });
}

function loc(files) {
  return files.reduce((n, f) => n + readFileSync(f, "utf8").split("\n").length, 0);
}

/** 旧引擎：按函数名包含技能 code 统计「技能专属代码」行数。 */
function oldPerSkill(codes) {
  const out = Object.fromEntries(codes.map((c) => [c, { loc: 0, fns: [] }]));
  // v1 crate 已删除时跳过「旧引擎专属 LOC」对比（S6 起该度量只作历史参考）。
  if (!existsSync(OLD_SRC)) return out;
  for (const f of walk(OLD_SRC).filter((p) => !p.includes(`${path.sep}tests${path.sep}`))) {
    const lines = readFileSync(f, "utf8").split("\n");
    let cur = null;
    let start = 0;
    const close = (end) => {
      if (!cur) return;
      for (const c of codes) {
        if (cur.includes(c)) {
          out[c].loc += end - start;
          out[c].fns.push(`${cur}:${end - start}`);
        }
      }
    };
    lines.forEach((line, i) => {
      const m = /^(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-z0-9_]+)/.exec(line);
      if (m) {
        close(i);
        cur = m[1];
        start = i;
      }
    });
    close(lines.length);
  }
  return out;
}

const newSrcFiles = walk(NEW_SRC);
const skillFiles = newSrcFiles.filter((f) => f.includes(`${path.sep}skills${path.sep}`) && !f.endsWith("mod.rs"));
// b0.rs 里合并实现了 7 个技能（其余文件 = 1 技能）。
const GROUP = { b0: ["jushou", "jiewei", "buqu", "fenji", "xingshang", "fangzhu", "songwei"] };
const files = skillFiles.map((f) => path.basename(f, ".rs"));
const codes = files.flatMap((c) => GROUP[c] ?? [c]);
const skillUnits = codes.length;
const kernelFiles = newSrcFiles.filter((f) => f.includes(`${path.sep}kernel${path.sep}`));
const pluginFiles = newSrcFiles.filter(
  (f) => !skillFiles.includes(f) && !kernelFiles.includes(f),
);

const newKernel = loc(kernelFiles);
const newSkills = loc(skillFiles);
const newPlugin = loc(pluginFiles);
const newTests = loc(walk(NEW_TESTS));
const testCount = walk(NEW_TESTS).reduce(
  (n, f) => n + (readFileSync(f, "utf8").match(/#\[test\]/g) ?? []).length,
  0,
);

// 牌结算（M1）：手写内核 LOC / 牌型数；牌定义是生成物，单独计数。
const cardPlayFile = newSrcFiles.find((f) => f.endsWith(path.join("kernel", "card_play.rs")));
const cardPlayLoc = cardPlayFile ? loc([cardPlayFile]) : 0;
const cardGenFile = newSrcFiles.find((f) => f.endsWith("cards_generated.rs"));
const cardGenSrc = cardGenFile ? readFileSync(cardGenFile, "utf8") : "";
const cardDefCount = (cardGenSrc.match(/^    Row \{/gm) ?? []).length;
const cardEffects = new Set(
  [...cardGenSrc.matchAll(/effect: Effect::(\w+)/g)].map((m) => m[1]).filter((e) => e !== "None"),
).size;
const cardTests = walk(NEW_TESTS)
  .filter((f) => path.basename(f) === "cards.rs")
  .reduce((n, f) => n + (readFileSync(f, "utf8").match(/#\[test\]/g) ?? []).length, 0);
const viewLoc = loc(newSrcFiles.filter((f) => f.endsWith(path.join("src", "view.rs"))));
const turnLoc = loc(newSrcFiles.filter((f) => f.endsWith(path.join("src", "turn.rs"))));
const viewTests = walk(NEW_TESTS)
  .filter((f) => path.basename(f) === "view_contract.rs")
  .reduce((n, f) => n + (readFileSync(f, "utf8").match(/#\[test\]/g) ?? []).length, 0);
const aiExample = path.join(projectRoot, "crates/engines/sanguosha2/examples/sgs2-selfplay.rs");
const aiLoc = existsSync(aiExample) ? readFileSync(aiExample, "utf8").split(String.fromCharCode(10)).length : 0;

const old = oldPerSkill(codes);
const oldTotal = Object.values(old).reduce((n, v) => n + v.loc, 0);

console.log("== 新 crate（sanguosha2） ==");
console.log(`  内核      ${String(newKernel).padStart(5)} 行（一次性，全部技能共用）`);
console.log(`  技能专属  ${String(newSkills).padStart(5)} 行 / ${skillUnits} 技能 = ${(newSkills / skillUnits).toFixed(0)} 行/技能`);
console.log(`  展示+插件 ${String(newPlugin).padStart(5)} 行`);
console.log(`  用例      ${String(newTests).padStart(5)} 行 / ${testCount} 条 = ${(testCount / skillUnits).toFixed(1)} 条/技能`);
console.log("== 牌结算（M1 内核） ==");
console.log(`  牌结算    ${String(cardPlayLoc).padStart(5)} 行 / ${cardEffects} 个牌型 = ${(cardPlayLoc / Math.max(cardEffects, 1)).toFixed(1)} 行/牌型`);
console.log(`  牌定义    ${String(cardDefCount).padStart(5)} 条（生成物，不计入手写 LOC）`);
console.log(`  牌用例    ${String(cardTests).padStart(5)} 条（tests/cards.rs）`);
console.log("== 视图 / AI（M3） ==");
console.log(`  视图      ${String(viewLoc).padStart(5)} 行 / ${viewTests} 条契约用例`);
console.log(`  回合驱动  ${String(turnLoc).padStart(5)} 行`);
console.log(`  AI 示例   ${String(aiLoc).padStart(5)} 行（examples/sgs2-selfplay.rs）`);
console.log("== 旧引擎（sanguosha）同技能专属代码 ==");
for (const c of codes) console.log(`  ${c.padEnd(12)} ${String(old[c].loc).padStart(4)} 行`);
console.log(`  合计        ${oldTotal} 行 / ${skillUnits} 技能 = ${(oldTotal / skillUnits).toFixed(0)} 行/技能`);
// ---------------------------------------------------------------------------
// 迁移趋势（B8 起）：注册技能数 / v1 已实现数 / 覆盖率 / 对齐单未答疑点
// ---------------------------------------------------------------------------
const registrySrc = path.join(NEW_SRC, "skills/mod.rs");
const registeredCount = (
  readFileSync(registrySrc, "utf8").match(/^\s+b[0-9a-z]+::[A-Z0-9_]+,$/gm) ?? []
).length;
const catalogPath = path.join(projectRoot, "docs/games/sanguosha/data/engine-catalog.json");
let v1Implemented = 0;
let v1Skills = 0;
if (existsSync(catalogPath)) {
  const catalog = JSON.parse(readFileSync(catalogPath, "utf8"));
  const names = new Set();
  for (const g of catalog.generals ?? []) {
    for (const sk of g.skills ?? []) names.add(typeof sk === "string" ? sk : sk.name);
  }
  v1Skills = names.size;
  // v1 的「已实现」事实源是 skills.rs 的 `SkillDef { … implemented: true }` 块（与 check:sgs-parity 同口径）。
  const v1SkillsSrc = path.join(OLD_SRC, "skills.rs");
  if (existsSync(v1SkillsSrc)) {
    const text = readFileSync(v1SkillsSrc, "utf8");
    const all = new Set();
    const done = new Set();
    for (const chunk of text.split("SkillDef {").slice(1)) {
      const code = /code:\s*"([a-z0-9_]+)"/.exec(chunk)?.[1];
      if (!code) continue;
      all.add(code);
      if (/implemented:\s*true/.test(chunk)) done.add(code);
    }
    v1Skills = all.size;
    v1Implemented = done.size;
  }
}
const alignDir = path.join(projectRoot, "docs/games/sanguosha/align");
const alignFiles = readdirSync(alignDir).filter((f) => /^B\d+\.json$/.test(f));
let alignQuestions = 0;
let alignUnanswered = 0;
const batchStats = [];
for (const f of alignFiles) {
  const spec = JSON.parse(readFileSync(path.join(alignDir, f), "utf8"));
  const qs = (spec.generals ?? []).flatMap((g) => (g.skills ?? []).flatMap((k) => k.questions ?? []));
  alignQuestions += qs.length;
  alignUnanswered += qs.filter((q) => !q.answer).length;
  const skills = (spec.generals ?? []).flatMap((g) => g.skills ?? []);
  const impl = skills.filter((k) => k.status === "implemented" || k.status === "verified").length;
  batchStats.push(`${spec.batch}=${impl}/${skills.length}`);
}
const skillLocByBatch = [
  "b0", "b1", "b1b", "b2", "b3", "b4", "b5", "b6", "b7", "b8",
].map((b) => {
  const f = path.join(NEW_SRC, "skills", `${b}.rs`);
  return existsSync(f) ? `${b}:${readFileSync(f, "utf8").split("\n").length}` : null;
}).filter(Boolean);

console.log("== 迁移进度（v1 → v2） ==");
console.log(`  v2 注册技能 ${String(registeredCount).padStart(4)} / v1 已实现 ${v1Implemented}（v1 技能总数 ${v1Skills}）`);
console.log(
  `  对齐单      ${String(alignFiles.length).padStart(4)} 批 / 疑点 ${alignQuestions} 条（未答 ${alignUnanswered}）`,
);
console.log(`  已实现分布  ${batchStats.join("  ")}`);
console.log(`  技能文件行数 ${skillLocByBatch.join("  ")}`);

console.log("== 结论 ==");
const ratio = (newSkills / skillUnits) / (oldTotal / skillUnits);
console.log(`  每技能专属 LOC 比值：${(ratio * 100).toFixed(0)}%（目标曾为 ≤50%）`);
console.log(`  用例密度：${(testCount / skillUnits).toFixed(1)} 条/技能（旧引擎 B2 走查前约 2 条/技能）`);
console.log("  注：LOC 不是主收益；主收益是 6 类缺陷的结构性消除（见 docs/games/sanguosha/spike-report.md）。");

if (process.argv.includes("--verbose")) {
  for (const c of codes) console.log(`  [fn] ${c}: ${old[c].fns.join(", ")}`);
}
