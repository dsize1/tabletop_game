#!/usr/bin/env node
// 内核 v2 度量（spike 报告用，非门禁）：新 crate 的 LOC/用例 vs 旧引擎「同技能专属代码」。
//
// 用法：node apps/scripts/check-sgs2-metrics.mjs [--verbose]

import { readdirSync, readFileSync, statSync } from "node:fs";
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

const old = oldPerSkill(codes);
const oldTotal = Object.values(old).reduce((n, v) => n + v.loc, 0);

console.log("== 新 crate（sanguosha2） ==");
console.log(`  内核      ${String(newKernel).padStart(5)} 行（一次性，全部技能共用）`);
console.log(`  技能专属  ${String(newSkills).padStart(5)} 行 / ${skillUnits} 技能 = ${(newSkills / skillUnits).toFixed(0)} 行/技能`);
console.log(`  展示+插件 ${String(newPlugin).padStart(5)} 行`);
console.log(`  用例      ${String(newTests).padStart(5)} 行 / ${testCount} 条 = ${(testCount / skillUnits).toFixed(1)} 条/技能`);
console.log("== 旧引擎（sanguosha）同技能专属代码 ==");
for (const c of codes) console.log(`  ${c.padEnd(12)} ${String(old[c].loc).padStart(4)} 行`);
console.log(`  合计        ${oldTotal} 行 / ${skillUnits} 技能 = ${(oldTotal / skillUnits).toFixed(0)} 行/技能`);
console.log("== 结论 ==");
const ratio = (newSkills / skillUnits) / (oldTotal / skillUnits);
console.log(`  每技能专属 LOC 比值：${(ratio * 100).toFixed(0)}%（目标曾为 ≤50%）`);
console.log(`  用例密度：${(testCount / skillUnits).toFixed(1)} 条/技能（旧引擎 B2 走查前约 2 条/技能）`);
console.log("  注：LOC 不是主收益；主收益是 6 类缺陷的结构性消除（见 docs/games/sanguosha/spike-report.md）。");

if (process.argv.includes("--verbose")) {
  for (const c of codes) console.log(`  [fn] ${c}: ${old[c].fns.join(", ")}`);
}
