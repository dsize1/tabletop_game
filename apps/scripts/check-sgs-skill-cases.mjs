#!/usr/bin/env node
// 门禁（S6：事实源切 v2）：每个「已实现武将」至少 1 条 v2 规则用例锚点。
//
// 口径：
// - 武将集合 = `engine-catalog.json` 的 live（147）/ 或 `generals_generated.rs` 的 Status::Live；
// - 用例集合 = `crates/engines/sanguosha2/tests/*.rs`（`#[test]` 函数体）；
// - 锚点 = 函数体出现该武将 code（`sgs_hero_N`）或该武将任一技能 code（带引号）。
//
// 说明：v1 的「每将 ≥3 条」在 v2 用「每技能 code 必须有 #[test]」覆盖
// （`npm run check:sgs2-registry`），本门禁只保证「每个武将有归属用例」，防漏测。
//
// 用法：node apps/scripts/check-sgs-skill-cases.mjs [--min 1] [--verbose]

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { loadEngineCatalog, projectRoot } from "./lib/sgs-catalog.mjs";
import { loadSkillNameCodes } from "./sgs-data-lib.mjs";

const args = process.argv.slice(2);
const verbose = args.includes("--verbose");
const minArg = args.indexOf("--min");
const MIN = minArg >= 0 ? Number(args[minArg + 1] ?? 1) : 1;

const TESTS_DIR = path.join(projectRoot, "crates/engines/sanguosha2/tests");

const nameToCode = loadSkillNameCodes();
const catalog = loadEngineCatalog();
const live = catalog.generals.filter((g) => g.status === "live");

// 收集全部测试体（文件 → 内容）。
let testFiles = [];
try {
  testFiles = readdirSync(TESTS_DIR)
    .filter((f) => f.endsWith(".rs"))
    .map((f) => ({ file: f, text: readFileSync(path.join(TESTS_DIR, f), "utf8") }));
} catch (e) {
  console.error(`[skill-cases] 读 v2 测试目录失败：${e.message}`);
  process.exit(1);
}

/** 该武将的锚点命中（返回命中的文件列表）。 */
function anchorsOf(g) {
  const codes = (g.skills ?? []).map((n) => nameToCode.get(n)).filter(Boolean);
  const hits = [];
  for (const { file, text } of testFiles) {
    if (text.includes(`"${g.code}"`)) {
      hits.push(file);
      continue;
    }
    if (codes.some((c) => text.includes(`"${c}"`))) hits.push(file);
  }
  return hits;
}

const problems = [];
let ok = 0;
const details = [];
for (const g of live) {
  const hits = anchorsOf(g);
  if (hits.length < MIN) {
    problems.push(`${g.code} ${g.name}：用例锚点 ${hits.length} < ${MIN}`);
  } else {
    ok += 1;
  }
  if (verbose) details.push(`${g.code} ${g.name}: ${hits.join(",") || "-"}`);
}

if (verbose) for (const d of details) console.log(`  ${d}`);

if (problems.length) {
  console.error(`[skill-cases] 失败 ${problems.length}/${live.length}：`);
  for (const p of problems.slice(0, 40)) console.error(`  - ${p}`);
  process.exit(1);
}
console.log(`[skill-cases] OK：${ok}/${live.length} 位 live 武将各有 ≥${MIN} 条 v2 用例锚点`);
